//! Locomotion: the keyboard into the player's Intent, each Intent into its
//! character controller, and the body turning to face where it's going.
//! Port of `character/locomotion.go`.

use bevy::{
    input::{ButtonInput, keyboard::KeyCode},
    prelude::*,
};
use std::f32::consts::PI;

use super::{
    Anim, Body, Character, CharacterController, Intent, Player, State, Traversal, clamp,
    horizontal, wrap_angle, yaw_of,
};

/// Whether the keyboard moves the player. A menu turns it off while it's
/// open.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Controls {
    pub enabled: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls { enabled: true }
    }
}

/// The way the player's camera faces, on the XZ plane. The keys move the
/// player relative to it, W away from the camera and A to its left. Unset,
/// it faces -Z.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct View {
    pub forward: Vec3,
}

/// `dir`, given as on the keys (-Z ahead, +X right), turned to go the same
/// way relative to `forward`.
pub fn relative(dir: Vec3, mut forward: Vec3) -> Vec3 {
    forward.y = 0.0;
    if forward.length() < 1e-6 {
        return dir;
    }
    let forward = forward.normalize();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    right * dir.x + forward * -dir.z
}

/// Turns the keyboard into the player's Intent: WASD or the arrows move
/// (relative to the View), Shift runs, Space jumps, E interacts, F punches
/// and Q picks up; T talks and G dances until pressed again or until the
/// player does anything else. With Controls off, the player stands still.
/// Without a keyboard (headless), it does nothing.
pub fn player_input(
    mut q: Query<&mut Intent, With<Player>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    controls: Res<Controls>,
    view: Res<View>,
) {
    if !controls.enabled {
        for mut intent in &mut q {
            *intent = Intent::default();
        }
        return;
    }
    let Some(k) = keys else {
        return;
    };
    let mut dir = Vec3::ZERO;
    if k.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if k.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    if k.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.z -= 1.0;
    }
    if k.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.z += 1.0;
    }
    if dir != Vec3::ZERO {
        dir = relative(dir.normalize(), view.forward);
    }
    let act = if k.just_pressed(KeyCode::KeyE) {
        Anim::Interact
    } else if k.just_pressed(KeyCode::KeyF) {
        Anim::Punch
    } else if k.just_pressed(KeyCode::KeyQ) {
        Anim::PickUp
    } else {
        Anim::Idle
    };
    let emote = if k.just_pressed(KeyCode::KeyT) {
        Anim::Talk
    } else if k.just_pressed(KeyCode::KeyG) {
        Anim::Dance
    } else {
        Anim::Idle
    };
    let slide = k.just_pressed(KeyCode::ControlLeft) || k.just_pressed(KeyCode::ControlRight);
    for mut intent in &mut q {
        // A held pose lasts until it's pressed again, or until anything
        // else is done.
        if emote != Anim::Idle && intent.hold == emote {
            intent.hold = Anim::Idle;
        } else if emote != Anim::Idle {
            intent.hold = emote;
        } else if dir != Vec3::ZERO
            || act != Anim::Idle
            || k.just_pressed(KeyCode::Space)
            || k.just_pressed(KeyCode::KeyR)
            || slide
            || k.just_pressed(KeyCode::KeyC)
        {
            intent.hold = Anim::Idle;
        }
        intent.move_dir = dir;
        intent.slide = intent.slide || slide;
        intent.roll = intent.roll || k.just_pressed(KeyCode::KeyR);
        intent.crouch = k.pressed(KeyCode::KeyC);
        intent.run = k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        if k.just_pressed(KeyCode::Space) {
            intent.jump = true;
        }
        if act != Anim::Idle {
            intent.act = act;
        }
    }
}

/// How much slower a body turns at full running speed than at a
/// standstill: the faster it goes, the wider it has to turn.
pub const RUN_TURN: f32 = 0.45;
/// The furthest a character's steps stray from where its body faces, and
/// the share of its speed left heading straight back the way it faces:
/// turning about, it slows and walks round in an arc.
pub const SLIP: f32 = 50.0 * PI / 180.0;
pub const CRAWL: f32 = 0.08;
/// How many times quicker than it speeds up a character slows down: feet
/// plant and stop it quicker than they push off.
pub const BRAKE: f32 = 2.0;
/// The furthest a character can veer, going or facing, from how it left
/// the ground, and how fast it can change speed up there (never faster
/// than it took off).
pub const AIR_STEER: f32 = 10.0 * PI / 180.0;
pub const AIR_ACCEL: f32 = 2.0;
/// As fast as a character goes on stairs, across the ground: a step at a
/// time, at about the capture's pace.
pub const STAIRS_WALK: f32 = 0.9;
pub const STAIRS_RUN: f32 = 1.8;
/// How many times quicker than it speeds up a character brakes for an
/// action: from a run to below a walk in about a third of a second.
pub const PULL_UP: f32 = 2.0;

/// Runs in FixedUpdate, feeding each Intent to its character controller.
/// Characters step along where their bodies face (see `stride`), ease
/// their speed up and down (see `ease`), pull up (and can't jump) for an
/// action, and in the air keep to the way they jumped (see `airborne`).
#[allow(clippy::type_complexity)]
pub fn locomote(
    mut q: Query<
        (
            &Character,
            &mut Intent,
            &mut CharacterController,
            &Traversal,
            Option<&Children>,
        ),
        Without<Body>,
    >,
    mut bodies: Query<(&mut State, &Transform), With<Body>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (c, mut intent, mut cc, traversal, children) in &mut q {
        if traversal.active() || traversal.impulse {
            intent.jump = false;
            continue;
        }
        let mut mv = intent.move_dir;
        mv.y = 0.0;
        let l = mv.length();
        if l > 1.0 {
            mv /= l;
        }
        let mut speed = c.walk_speed;
        if intent.run || !cc.grounded {
            // In the air, letting go of Run doesn't slow it: airborne keeps
            // the speed it jumped at.
            speed = c.run_speed;
        }
        let mut acting = false;
        let mut body: Option<Mut<State>> = None;
        let child = children.and_then(|children| children.iter().find(|c| bodies.contains(*c)));
        if let Some(child) = child
            && let Ok((mut st, tr)) = bodies.get_mut(child)
        {
            acting = st.current.one_shot();
            if !cc.grounded && !st.aloft {
                // Just left the ground: that's the way it's going now.
                st.launch = Vec3::new(cc.walk.x, 0.0, cc.walk.z);
                st.launch_yaw = yaw_of(tr.rotation);
            }
            st.aloft = !cc.grounded;
            let l = mv.length();
            if l > 0.0 && !st.aloft {
                let (dir, share) = stride(yaw_of(tr.rotation), mv);
                mv = dir * (l * share);
            }
            body = Some(st);
        }
        if let Some(st) = &body
            && st.stairs != 0
            && cc.grounded
        {
            let top = if intent.run { STAIRS_RUN } else { STAIRS_WALK };
            speed = speed.min(top);
        }
        let mut want = mv * speed;
        if traversal.land > 0.0 {
            want = Vec3::ZERO; // taking the landing from a wall kick
        }
        if traversal.bounced {
            // Off a wall kick it flies the way it bounced until it lands.
        } else if body.as_ref().is_some_and(|st| st.aloft) {
            let launch = body.as_ref().map(|st| st.launch).unwrap_or_default();
            cc.walk = approach(cc.walk, airborne(launch, want), AIR_ACCEL * dt);
        } else if acting {
            cc.walk = ease(
                cc.walk,
                Vec3::ZERO,
                PULL_UP * c.ease,
                PULL_UP * accel(c.accel),
                dt,
            );
        } else {
            let k = if want.length() < cc.walk.length() {
                BRAKE
            } else {
                1.0
            };
            cc.walk = ease(cc.walk, want, k * c.ease, k * accel(c.accel), dt);
        }
        if intent.jump && cc.grounded && !acting {
            cc.velocity.y = c.jump_speed;
        }
        intent.jump = false;
    }
}

/// Where a character that left the ground going `launch` (on the XZ
/// plane) heads when it wants to go `want`: it veers at most AIR_STEER from
/// launch and never speeds up. Letting go keeps it going; pulling back past
/// square on only checks it, to half its speed. Jumping on the spot, it
/// stays put.
pub fn airborne(launch: Vec3, want: Vec3) -> Vec3 {
    let top = launch.length();
    let mut speed = want.length().min(top);
    if top < 0.1 {
        return Vec3::ZERO;
    }
    if speed == 0.0 {
        return launch;
    }
    let from = launch.x.atan2(launch.z);
    let mut off = wrap_angle(want.x.atan2(want.z) - from);
    if off.abs() > PI / 2.0 {
        off = 0.0;
        speed = top / 2.0;
    }
    off = clamp(off, -AIR_STEER, AIR_STEER);
    Vec3::new(speed * (from + off).sin(), 0.0, speed * (from + off).cos())
}

/// `a`, or no limit if it's unset.
pub fn accel(a: f32) -> f32 {
    if a <= 0.0 { f32::MAX } else { a }
}

/// How close (units per second) `ease` gets before it's there.
pub const SNAP: f32 = 0.02;

/// Moves `v` toward `want` over `dt`: it closes the share 1 - e^(-rate·dt)
/// of the gap (so it slows as it closes in), no faster than `most` per
/// second, and snaps there once within SNAP. With no rate, it goes at most.
pub fn ease(v: Vec3, want: Vec3, rate: f32, most: f32, dt: f32) -> Vec3 {
    let d = want - v;
    let l = d.length();
    if l <= SNAP {
        return want;
    }
    let mut step = most * dt;
    if rate > 0.0 {
        step = step.min(l * (1.0 - (-(rate as f64) * dt as f64).exp()) as f32);
    }
    v + d * (step.min(l) / l)
}

/// Moves `v` toward `want` by at most `step`.
pub fn approach(v: Vec3, want: Vec3, step: f32) -> Vec3 {
    let d = want - v;
    let l = d.length();
    if l > step { v + d * (step / l) } else { want }
}

/// Which way a body facing `yaw` steps when heading along `mv` (on the XZ
/// plane), and the share of its speed it can put into it: its steps stray
/// at most SLIP from where it faces, and it slows the further it has to
/// turn, sharply past a quarter turn, to CRAWL of its speed heading
/// straight back: turning about, it all but stops and pivots.
pub fn stride(yaw: f32, mv: Vec3) -> (Vec3, f32) {
    let off = wrap_angle(mv.x.atan2(mv.z) - yaw);
    let step = yaw + clamp(off, -SLIP, SLIP);
    let dir = Vec3::new(step.sin(), 0.0, step.cos());
    let ahead = (1.0 + off.cos()) / 2.0;
    (dir, CRAWL + (1.0 - CRAWL) * ahead * ahead)
}

/// Turns each body toward where its character wants to go, no faster than
/// its turn rate, speeding up into the turn and easing out of it; in the
/// air, no further than AIR_STEER from how it took off. With nowhere to
/// go, or while an action plays, a turn under way winds down. The models
/// face +Z.
#[allow(clippy::type_complexity)]
pub fn face(
    mut bodies: Query<(&ChildOf, &mut Transform, &mut State), With<Body>>,
    roots: Query<(&Character, &Intent, &CharacterController, &Traversal)>,
    time: Res<Time>,
    controls: Res<Controls>,
) {
    let dt = time.delta_secs();
    for (parent, mut tr, mut st) in &mut bodies {
        let Ok((c, intent, cc, traversal)) = roots.get(parent.parent()) else {
            continue;
        };
        if traversal.active() {
            st.turn = 0.0;
            continue;
        }
        if traversal.kick > 0.0 {
            st.turn = 0.0;
            if controls.enabled {
                // Off the wall it faces the way it bounced, from the start.
                let target = Quat::from_axis_angle(Vec3::Y, st.launch_yaw);
                tr.rotation = tr.rotation.slerp(target, (dt * 28.0).min(1.0));
            }
            continue;
        }
        if traversal.bounced {
            st.turn = 0.0;
            continue;
        }
        let mut yaw = yaw_of(tr.rotation);
        if (intent.move_dir.x == 0.0 && intent.move_dir.z == 0.0) || st.current.one_shot() {
            if st.turn == 0.0 {
                continue; // leave the body be: a menu may be turning it
            }
            st.turn = toward(st.turn, 0.0, accel(c.turn_accel) * dt);
            yaw += st.turn * dt;
        } else {
            let top = turn_rate(c, horizontal(cc.walk).length());
            let mut target = intent.move_dir.x.atan2(intent.move_dir.z);
            if st.aloft {
                target = st.launch_yaw
                    + clamp(wrap_angle(target - st.launch_yaw), -AIR_STEER, AIR_STEER);
            }
            let (y, rate) = turn_toward(yaw, st.turn, target, top, accel(c.turn_accel), dt);
            yaw = y;
            st.turn = rate;
        }
        tr.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
    }
}

/// The fastest `c`'s body turns going at `speed`: turn_speed at a
/// standstill, down to RUN_TURN of it at full running speed.
pub fn turn_rate(c: &Character, speed: f32) -> f32 {
    if c.run_speed <= 0.0 {
        return c.turn_speed;
    }
    c.turn_speed * (1.0 - (1.0 - RUN_TURN) * clamp(speed / c.run_speed, 0.0, 1.0))
}

/// Turns from `yaw`, turning at `rate` (radians per second), toward
/// `target` over `dt`: speeding up by at most `accel`, no faster than
/// `top`, and slowing in time to stop on target. Returns the new yaw and
/// rate.
pub fn turn_toward(yaw: f32, rate: f32, target: f32, top: f32, accel: f32, dt: f32) -> (f32, f32) {
    let mut off = wrap_angle(target - yaw);
    if off == 0.0 && rate == 0.0 {
        return (yaw, 0.0);
    }
    // Straight behind, keep turning the way it already is.
    if off.abs() > PI - 1e-3 && rate != 0.0 && (off > 0.0) != (rate > 0.0) {
        off = -off;
    }
    // The fastest it can go and still stop on target.
    let mut want = top.min((2.0 * accel * off.abs()).sqrt());
    if off < 0.0 {
        want = -want;
    }
    let rate = toward(rate, want, accel * dt);
    let step = rate * dt;
    if (step > 0.0) == (off > 0.0) && step.abs() >= off.abs() {
        return (target, 0.0); // there
    }
    (yaw + step, rate)
}

/// Moves `v` toward `want` by at most `step`.
pub fn toward(v: f32, want: f32, step: f32) -> f32 {
    let d = want - v;
    if d.abs() > step {
        if d > 0.0 { v + step } else { v - step }
    } else {
        want
    }
}
