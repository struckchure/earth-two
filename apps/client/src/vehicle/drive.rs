//! Driving: the keys steer the player's vehicle, nobody's vehicle parks
//! itself, and each vehicle is drawn between physics steps. Port of
//! `vehicle/drive.go`. Getting in and out needs the character: here it is
//! [`enter`], [`nearest_seat`] and the [`EnteredVehicle`] and
//! [`LeftVehicle`] messages the character port seats and stands people by.

use avian3d::prelude::*;
use bevy::{ecs::entity::Entities, prelude::*};

use super::{
    Drivable, Seats, WheelOf,
    handling::{Handling, SeatAnim},
    sim::{VehicleInput, VehicleState},
    spec::{self, SeatSpec, chase_distance, seat_pose, seat_reach, yaw_of},
};
use crate::character;

/// m/s: any faster, and it won't let you out.
pub const EXIT_SPEED: f32 = 4.0;
/// m/s backwards.
pub const REVERSE_CAP: f32 = 9.0;
/// Seconds R is held to right a vehicle.
pub const RIGHTING: f32 = 1.0;
/// m: below this it's fallen out of the world.
pub const LOST: f32 = -40.0;
/// Seconds a note stays up.
pub const NOTE_FOR: f32 = 2.0;
/// How close (m) the player must be to a seat to get in.
pub const REACH: f32 = 2.5;

/// What the driver asks of the vehicle this frame, from the keys: W and S
/// (`forward` ±1), A and D (`steer` ±1, D positive), Space held, R held,
/// E and H just pressed. The interaction port fills it; `exit` and
/// `headlamps` are taken when acted on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Controls {
    pub forward: f32,
    pub steer: f32,
    pub hand_brake: bool,
    pub right: bool,
    pub exit: bool,
    pub headlamps: bool,
}

/// What the HUD offers the player: `key` to do `text` ("E", "Drive buggy"),
/// or, with no key, just a note ("Slow down to get out").
#[derive(Resource, Debug, Clone, Default)]
pub struct Prompt {
    pub key: String,
    pub text: String,
    /// A passing message, shown for a couple of seconds.
    pub note: String,
    note_left: f32,
}

impl Prompt {
    /// The note to show now, if any.
    pub fn noting(&self) -> &str {
        if self.note_left > 0.0 { &self.note } else { "" }
    }

    /// Shows text for a couple of seconds.
    pub fn note(&mut self, text: &str) {
        self.note = text.to_string();
        self.note_left = NOTE_FOR;
    }
}

/// What the player is driving, for the camera and the HUD.
#[derive(Resource, Debug, Clone, Default)]
pub struct Driving {
    pub vehicle: Option<Entity>, // None on foot
    pub name: String,
    /// The vehicle's drawn pose this frame.
    pub pose: (Vec3, Quat),
    /// How far behind to follow.
    pub camera: f32,
    pub speed: f32, // m/s, forward
    pub gear: i32,
    pub headlamps: bool,
}

impl Driving {
    /// Whether the player is driving.
    pub fn active(&self) -> bool {
        self.vehicle.is_some()
    }
}

/// Someone sat in a vehicle's driving seat: what `character.Sit` was
/// given. The character port seats them.
#[derive(Message, Debug, Clone)]
pub struct EnteredVehicle {
    pub driver: Entity,
    pub vehicle: Entity,
    pub pose: (Vec3, Quat),
    pub anim: SeatAnim,
    /// Shut in a cab: out of sight.
    pub hidden: bool,
    /// Where a rider holds the bars and rests their feet, in the seat's
    /// frame.
    pub hands: Vec<Vec3>,
    pub feet: Vec<Vec3>,
}

/// Someone got out: where they stand, facing the vehicle's way. The
/// character port stands them there.
#[derive(Message, Debug, Clone, Copy)]
pub struct LeftVehicle {
    pub driver: Entity,
    pub vehicle: Entity,
    pub feet: Vec3,
    pub yaw: f32,
}

/// How `seat` has its sitter sit.
pub fn seat_anim(seat: &SeatSpec, h: &Handling) -> SeatAnim {
    match seat.pose.as_str() {
        "ride" => return SeatAnim::Ride,
        "drive" | "inside" => return SeatAnim::Drive,
        _ => {}
    }
    if h.seat != SeatAnim::Idle {
        return h.seat;
    }
    SeatAnim::Drive
}

/// The free driving seat nearest `feet`, within reach: what `offer` looks
/// for when the player stands still on the ground.
pub fn nearest_seat<'a>(
    feet: Vec3,
    cars: impl IntoIterator<Item = (Entity, &'a Drivable, &'a Seats, &'a Transform)>,
) -> Option<Entity> {
    let mut best = None;
    let mut best_dist = REACH;
    for (e, d, seats, tr) in cars {
        if d.driver.is_some() {
            continue;
        }
        let Some(seat) = seats.0.first() else {
            continue;
        };
        let (seat, _) = seat_pose(tr.translation, tr.rotation, seat);
        let dy = seat.y - feet.y;
        if !(-1.0..=2.0).contains(&dy) {
            continue;
        }
        let dist = (seat.x - feet.x).hypot(seat.z - feet.z);
        if dist < best_dist {
            best = Some(e);
            best_dist = dist;
        }
    }
    best
}

/// Puts `driver` in the driving seat of `car`: it wakes, and the character
/// port is told where to sit.
#[allow(clippy::too_many_arguments)]
pub fn enter(
    commands: &mut Commands,
    car: Entity,
    d: &mut Drivable,
    seats: &Seats,
    tr: &Transform,
    driver: Entity,
    driving: &mut Driving,
    entered: &mut MessageWriter<EnteredVehicle>,
) {
    let Some(seat) = seats.0.first() else {
        return;
    };
    entered.write(EnteredVehicle {
        driver,
        vehicle: car,
        pose: seat_pose(tr.translation, tr.rotation, seat),
        anim: seat_anim(seat, &d.handling),
        hidden: seat.pose == "inside",
        hands: seat_reach(seat, &seat.grips),
        feet: seat_reach(seat, &seat.pegs),
    });
    d.driver = Some(driver);
    d.still = 0.0;
    d.upset = 0.0;
    d.righting = 0.0;
    // Required easing opt-out components remain after removing their marker,
    // so remove all of them when waking a parked vehicle.
    commands
        .entity(car)
        .insert((RigidBody::Dynamic, SleepingDisabled))
        .remove::<(
            NoTransformEasing,
            NoTranslationEasing,
            NoRotationEasing,
            NoScaleEasing,
        )>();
    driving.vehicle = Some(car);
}

/// Drives the player's vehicle from the keys: W and S throttle and brake
/// (and reverse, from a stop), A and D steer, Space holds the hand brake,
/// E gets out once it's slow enough, and R rights it when it's over.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn steer(
    mut cars: Query<
        (
            Entity,
            &mut Drivable,
            &Seats,
            &mut Controls,
            &mut VehicleInput,
            &VehicleState,
            &mut Transform,
            &mut Position,
            &mut Rotation,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&RigidBodyColliders>,
        ),
        Without<Collider>,
    >,
    entities: &Entities,
    obstacles: Query<(&Collider, &Position, &Rotation), With<Collider>>,
    spatial: SpatialQuery,
    controls: Res<character::Controls>,
    time: Res<Time>,
    mut prompt: ResMut<Prompt>,
    mut left: MessageWriter<LeftVehicle>,
) {
    let dt = time.delta_secs();
    prompt.key.clear();
    prompt.text.clear();
    for (
        e,
        mut d,
        seats,
        mut keys,
        mut input,
        st,
        mut tr,
        mut pos,
        mut rot,
        mut lin,
        mut ang,
        colliders,
    ) in &mut cars
    {
        let Some(driver) = d.driver else {
            continue;
        };
        if !entities.contains(driver) {
            // Gone: despawned, or stood up by something else.
            d.driver = None;
            *input = VehicleInput {
                hand_brake: 1.0,
                ..default()
            };
            continue;
        }
        if !controls.enabled {
            continue;
        }
        let h = d.handling.clone();
        let speed = st.speed;

        let mut throttle = keys.forward.clamp(-1.0, 1.0);
        let mut brake = 0.0;
        if (throttle > 0.0 && speed < -1.5) || (throttle < 0.0 && speed > 1.5) {
            // Against the way it's going: brake first, then go.
            throttle = 0.0;
            brake = 1.0;
        } else if (throttle > 0.0 && h.top > 0.0 && speed > h.top)
            || (throttle < 0.0 && speed < -REVERSE_CAP)
        {
            throttle = 0.0;
        }
        let mut want = keys.steer.clamp(-1.0, 1.0);
        if h.steer_fade > 0.0 {
            want /= 1.0 + speed.abs() / h.steer_fade;
        }
        let mut rate = 2.5;
        if want == 0.0 || want * d.steer < 0.0 {
            rate = 4.0; // back to the middle more quickly
        }
        d.steer += (want - d.steer).clamp(-rate * dt, rate * dt);
        input.forward = throttle;
        input.right = d.steer;
        input.brake = brake;
        input.hand_brake = if keys.hand_brake { 1.0 } else { 0.0 };

        // Over on its side or roof: hold R to right it.
        let up = rot.0 * Vec3::Y;
        if up.y < 0.35 {
            d.upset += dt;
        } else {
            d.upset = 0.0;
            d.righting = 0.0;
        }
        if d.upset > 1.0 {
            prompt.key = "R".into();
            prompt.text = format!("Hold to right the {}", d.name);
            if keys.right {
                d.righting += dt;
            } else {
                d.righting = 0.0;
            }
            if d.righting >= RIGHTING {
                let at = pos.0 + Vec3::Y * 1.5;
                let turn = Quat::from_axis_angle(Vec3::Y, yaw_of(rot.0));
                teleport(&mut tr, &mut pos, &mut rot, &mut lin, &mut ang, at, turn);
                d.upset = 0.0;
                d.righting = 0.0;
            }
        }
        if pos.0.y < LOST {
            let (at, turn) = d.home;
            teleport(
                &mut tr,
                &mut pos,
                &mut rot,
                &mut lin,
                &mut ang,
                at + Vec3::Y,
                turn,
            );
        }

        if prompt.key.is_empty() && speed.abs() <= EXIT_SPEED {
            prompt.key = "E".into();
            prompt.text = "Get out".into();
        }
        if !keys.exit {
            continue;
        }
        keys.exit = false;
        if speed.abs() > EXIT_SPEED {
            prompt.note("Slow down to get out");
            continue;
        }
        let mut excluded = vec![e];
        if let Some(colliders) = colliders {
            excluded.extend(colliders.iter());
        }
        let filter = SpatialQueryFilter::from_excluded_entities(excluded);
        let exits = seats.0.first().map(|s| s.exits.clone()).unwrap_or_default();
        let mut stood = false;
        for exit in exits {
            let spot = pos.0 + rot.0 * Vec3::from(exit);
            let Some(hit) = spatial.cast_ray(spot + Vec3::Y * 1.5, Dir3::NEG_Y, 4.0, true, &filter)
            else {
                continue;
            };
            if hit.normal.y < 0.6 {
                continue;
            }
            let feet = spot + Vec3::Y * (1.5 - hit.distance);
            let standing = Collider::capsule(0.3, 1.8 - 0.6);
            let intersections = spatial.shape_intersections(
                &standing,
                feet + Vec3::Y * 0.92,
                Quat::IDENTITY,
                &filter,
            );
            if intersections.iter().any(|entity| {
                let Ok((shape, pos, rot)) = obstacles.get(*entity) else {
                    return true;
                };
                // Confirm penetration: GJK intersection tests can report a
                // separated capsule against a large ground box as intersecting.
                let result = avian3d::collision::collider::contact_query::contact(
                    shape,
                    pos.0,
                    *rot,
                    &standing,
                    feet + Vec3::Y * 0.92,
                    Quat::IDENTITY,
                    0.0,
                );
                match result {
                    Ok(Some(contact)) => contact.penetration > 0.0,
                    Ok(None) => false,
                    Err(_) => true,
                }
            }) {
                continue;
            }
            left.write(LeftVehicle {
                driver,
                vehicle: e,
                feet,
                yaw: yaw_of(rot.0),
            });
            d.driver = None;
            d.still = 0.0;
            d.steer = 0.0;
            *input = VehicleInput {
                brake: 1.0,
                hand_brake: 1.0,
                ..default()
            };
            stood = true;
            break;
        }
        if !stood {
            prompt.note("No room to get out here");
        }
    }
    if prompt.note_left > 0.0 {
        prompt.note_left -= dt;
    }
}

/// Moves a body without sliding: its physics pose and its drawn one, and
/// stops it.
fn teleport(
    tr: &mut Transform,
    pos: &mut Position,
    rot: &mut Rotation,
    lin: &mut LinearVelocity,
    ang: &mut AngularVelocity,
    at: Vec3,
    turn: Quat,
) {
    pos.0 = at;
    rot.0 = turn;
    tr.translation = at;
    tr.rotation = turn;
    lin.0 = Vec3::ZERO;
    ang.0 = Vec3::ZERO;
}

/// Draws each vehicle between physics steps (Avian interpolates the root's
/// Transform), its wheels where they were simulated, and says what the
/// player is driving.
#[allow(clippy::type_complexity)]
pub fn present(
    cars: Query<(
        Entity,
        &Drivable,
        &Transform,
        &RigidBody,
        &VehicleState,
        Option<&Children>,
    )>,
    mut wheels: Query<(&WheelOf, &mut Transform), Without<Drivable>>,
    mut driving: ResMut<Driving>,
) {
    driving.vehicle = None;
    for (e, d, root, rb, st, children) in &cars {
        if rb.is_dynamic()
            && !st.wheels.is_empty()
            && let Some(children) = children
        {
            for child in children.iter() {
                if let Ok((w, mut tr)) = wheels.get_mut(child)
                    && w.index < st.wheels.len()
                {
                    *tr = st.wheels[w.index].transform;
                }
            }
        }
        if d.driver.is_none() {
            continue;
        }
        *driving = Driving {
            vehicle: Some(e),
            name: d.name.clone(),
            pose: (root.translation, root.rotation),
            camera: chase_distance(&d.spec),
            speed: st.speed,
            gear: st.gear,
            headlamps: driving.headlamps,
        };
    }
}

/// Parks a vehicle nobody's driving: it holds the brakes, and once it's
/// come to rest (or left the ground that's simulated), it's made Static.
#[allow(clippy::type_complexity)]
pub fn settle(
    mut commands: Commands,
    mut cars: Query<(
        Entity,
        &mut Drivable,
        &RigidBody,
        &mut VehicleInput,
        &VehicleState,
        &Position,
    )>,
    ground: Option<Res<super::GroundCover>>,
    physics_time: Res<Time<Physics>>,
    time: Res<Time>,
) {
    if physics_time.is_paused() {
        return;
    }
    let dt = time.delta_secs();
    let covered = |x: f32, z: f32| match ground.as_ref().and_then(|g| g.covered.as_ref()) {
        Some(f) => f(x, z),
        None => true,
    };
    for (e, mut d, rb, mut input, st, pos) in &mut cars {
        if d.driver.is_some() || !rb.is_dynamic() {
            continue;
        }
        *input = VehicleInput {
            brake: 1.0,
            hand_brake: 1.0,
            ..default()
        };
        if st.speed.abs() < 0.3 {
            d.still += dt;
        } else {
            d.still = 0.0;
        }
        if d.still > 1.5 || !covered(pos.0.x, pos.0.z) {
            commands
                .entity(e)
                .insert((RigidBody::Static, NoTransformEasing))
                .remove::<SleepingDisabled>();
            d.still = 0.0;
        }
    }
}

/// A wheel's model where it's modelled, for a parked vehicle.
pub fn parked_wheel(w: &spec::WheelSpec) -> Transform {
    spec::parked_wheel(w)
}
