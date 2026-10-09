//! Traversal: slides, rolls, crouching, vaults, mantles, ladders and wall
//! kicks. Authoritative fixed-step state; animations follow it. Port of
//! `character/traversal.go`.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::PI;

use super::{
    Anim, Body, CAPSULE_HEIGHT, Character, CharacterController, CharacterPhysics, Controls, Intent,
    RayHit, State, clamp, direction, horizontal, wrap_angle,
};
use crate::world::Ladder;

/// Matches the wall kick clip: one foot taps the wall and pushes off, and
/// the body turns away from it in the air.
pub const WALL_KICK_TIME: f32 = 8.0 / 30.0;

/// The shares of a roll and a slide after which a character still
/// sprinting runs straight on: by then its feet are under it in a stride,
/// and the rest is getting up to stand. (The slide's clip gets up by
/// itself; see SLIDE_FRAMES in tools/makehuman/traversal.py.)
pub const ROLL_RELEASE: f32 = 0.6;
pub const SLIDE_RELEASE: f32 = 0.875;

/// A slide goes the way the character was running, and steers at most
/// SLIDE_STEER either side of that, turning at most SLIDE_TURN radians a
/// second. Pulling back further than SLIDE_BACK from the way it's going
/// doesn't steer it: it can't turn round.
pub const SLIDE_STEER: f32 = 45.0 * PI / 180.0;
pub const SLIDE_TURN: f32 = 1.5;
pub const SLIDE_BACK: f32 = 120.0 * PI / 180.0;

/// The way a slide going along `current`, which started heading the yaw
/// `heading`, goes after `dt` seconds of wanting to go `mv`.
pub fn steer_slide(current: Vec3, heading: f32, mv: Vec3, dt: f32) -> Vec3 {
    let mv = horizontal(mv);
    if mv == Vec3::ZERO {
        return current;
    }
    let mut yaw = current.x.atan2(current.z);
    let to = mv.x.atan2(mv.z);
    if wrap_angle(to - yaw).abs() > SLIDE_BACK {
        return current;
    }
    let want = heading + clamp(wrap_angle(to - heading), -SLIDE_STEER, SLIDE_STEER);
    yaw += clamp(wrap_angle(want - yaw), -SLIDE_TURN * dt, SLIDE_TURN * dt);
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// Distances, seconds and speeds used by every character.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TraversalConfig {
    pub low_height: f32,
    pub slide_min: f32,
    pub slide_speed: f32,
    pub slide_time: f32,
    pub roll_distance: f32,
    pub roll_time: f32,
    pub ladder_speed: f32,
    pub kick_speed: f32,
    pub kick_up: f32,
    pub guard: f32,
    /// The slowest it can be going to kick off a wall: a sprint.
    pub kick_min: f32,
    /// Seconds after landing from a wall kick before it moves off, unless
    /// it's still sprinting.
    pub land_delay: f32,
    /// How far the body's surface may be from a box, a ladder or a wall and
    /// still use it: near enough that the hands and feet meet it.
    pub reach: f32,
    pub ladder_reach: f32,
    pub wall_reach: f32,
}

impl Default for TraversalConfig {
    fn default() -> Self {
        TraversalConfig {
            low_height: 0.9,
            slide_min: 3.0,
            slide_speed: 6.5,
            slide_time: 0.8,
            roll_distance: 2.0,
            roll_time: 0.7,
            ladder_speed: 0.6,
            reach: 0.3,
            ladder_reach: 0.25,
            wall_reach: 0.35,
            kick_speed: 3.2,
            kick_up: 3.6,
            guard: 0.15,
            kick_min: 4.0,
            land_delay: 0.15,
        }
    }
}

/// Authoritative fixed-step traversal state; animations follow it.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Traversal {
    pub mode: Anim,
    pub elapsed: f32,
    pub duration: f32,
    pub guard: f32,
    pub detach: f32,
    pub direction: Vec3,
    pub speed: f32,
    pub start: Vec3,
    pub lift: Vec3,
    pub end: Vec3,
    pub ladder: Option<Entity>,
    pub last_wall: Option<Entity>,
    pub wall_normal: Vec3,
    pub wall_point: Vec3,
    pub wall_locked: bool,
    pub kick: f32,
    /// The wall is on the right: the right foot kicks.
    pub kick_right: bool,
    /// In the air off a wall kick: it can't steer until it lands.
    pub bounced: bool,
    /// Seconds left of land_delay.
    pub land: f32,
    pub impulse: bool,
    /// Seconds left to face an obstacle it's been asked to get over.
    pub queued: f32,
    pub hint: String,
    /// Climb cycles, driven by distance, not animation time.
    pub phase: f32,
    pub rung_spacing: f32,
    pub exit_top: bool,
    /// The way a slide started, as a yaw: it steers only so far from it.
    pub heading: f32,
}

impl Traversal {
    pub fn active(&self) -> bool {
        self.mode != Anim::Idle
    }
}

/// A ladder's rung spacing, where it differs from the authored clip's 0.3
/// m; goes beside `world::Ladder`, which has none.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct RungSpacing(pub f32);

const DEFAULT_RUNG_SPACING: f32 = 0.3;

fn ease_traversal(u: f32) -> f32 {
    let u = clamp(u, 0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The exit clip is authored against these (tools/makehuman/traversal.py):
/// the shares of a top exit by which the body has climbed to within
/// LADDER_EXIT_HOP of the landing, hopped up level with it, and crossed
/// onto it; how long it takes; and how far it climbs on before the hop
/// (LADDER_EXIT_RISE), which is how far below it starts. LADDER_FOOT is how
/// far up a ladder the climb's cycle starts, the left foot leaving its
/// rung: the clip's feet are on the rungs for ladders whose first rung is
/// 30 cm up, 30 cm apart.
pub const LADDER_EXIT_CLIMB: f32 = 42.0 / 90.0;
pub const LADDER_EXIT_UP: f32 = 63.0 / 90.0;
pub const LADDER_EXIT_CROSS: f32 = 81.0 / 90.0;
pub const LADDER_EXIT_HOP: f32 = 0.91;
pub const LADDER_EXIT_TIME: f32 = 3.0;
pub const LADDER_EXIT_RISE: f32 = 0.84;
pub const LADDER_FOOT: f32 = 0.21;

/// All of it at a steady pace: the clip's own motion is laid over the top.
pub fn ladder_exit_position(s: &Traversal, u: f32) -> Vec3 {
    if !s.exit_top {
        return s.start.lerp(s.end, ease_traversal(u));
    }
    let mut hop = s.lift;
    hop.y = s.start.y.max(hop.y - LADDER_EXIT_HOP);
    if u < LADDER_EXIT_CLIMB {
        return s.start.lerp(hop, u / LADDER_EXIT_CLIMB);
    }
    if u < LADDER_EXIT_UP {
        return hop.lerp(
            s.lift,
            (u - LADDER_EXIT_CLIMB) / (LADDER_EXIT_UP - LADDER_EXIT_CLIMB),
        );
    }
    s.lift.lerp(
        s.end,
        ((u - LADDER_EXIT_UP) / (LADDER_EXIT_CROSS - LADDER_EXIT_UP)).min(1.0),
    )
}

/// Starts the stand-up after a slide or roll.
pub fn stand_up(s: &mut Traversal, cc: &mut CharacterController) {
    s.mode = Anim::StandUp;
    s.elapsed = 0.0;
    s.duration = 0.28;
    cc.walk = Vec3::ZERO;
}

/// Lift vertically before passing across the obstacle, then settle onto
/// its top/landing.
fn path_position(s: &Traversal, u: f32) -> Vec3 {
    let across = Vec3::new(s.end.x, s.lift.y, s.end.z);
    if u < 0.4 {
        s.start.lerp(s.lift, u / 0.4)
    } else if u < 0.85 {
        s.lift.lerp(across, (u - 0.4) / 0.45)
    } else {
        across.lerp(s.end, (u - 0.85) / 0.15)
    }
}

fn clear_path(
    p: &CharacterPhysics,
    e: Entity,
    cc: &CharacterController,
    from: Vec3,
    to: Vec3,
) -> bool {
    if p.overlap_capsule_excluding(to, cc.radius, cc.height, e) {
        return false;
    }
    p.sweep_capsule_excluding(from, to - from, cc.radius, cc.height, e)
        .is_none()
}

fn end_traversal(s: &mut Traversal, cc: &mut CharacterController) {
    s.mode = Anim::Idle;
    s.elapsed = 0.0;
    cc.controlled = false;
    cc.walk = Vec3::ZERO;
}

fn refresh_launch(st: &mut State, cc: &CharacterController, dir: Vec3) {
    st.launch = horizontal(cc.walk);
    st.launch_yaw = dir.x.atan2(dir.z);
    st.aloft = true;
}

/// How squarely (the cosine of the angle off it) a character has to face
/// the way over an obstacle to start over it, and how long it has, asked
/// to, to turn that far.
pub const OBSTACLE_FACING: f32 = 0.94;
pub const OBSTACLE_WAIT: f32 = 0.8;
/// How squarely (the cosine again) a character in the air has to be
/// heading into a wall to climb onto the top of it.
pub const AIR_CLIMB_ANGLE: f32 = 0.5;

/// The ladder as the traversal reads it: the world's, with its rung
/// spacing.
#[derive(Clone, Copy)]
struct Rungs {
    ladder: Ladder,
    spacing: f32,
}

impl Rungs {
    fn axis(&self) -> Vec3 {
        direction(self.ladder.top - self.ladder.bottom)
    }

    fn length(&self) -> f32 {
        self.ladder.bottom.distance(self.ladder.top)
    }
}

/// The traversal state machine, in FixedUpdate before `locomote`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn traverse(
    mut q: Query<
        (
            Entity,
            &Character,
            &mut Intent,
            &mut CharacterController,
            &mut Transform,
            &mut Traversal,
            &TraversalConfig,
            Option<&Children>,
        ),
        Without<Body>,
    >,
    mut bodies: Query<(&mut State, &mut Transform), With<Body>>,
    ladders: Query<(Entity, &Ladder, Option<&RungSpacing>)>,
    p: CharacterPhysics,
    time: Res<Time>,
    physics_time: Res<Time<Physics>>,
    controls: Res<Controls>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || physics_time.is_paused() || !controls.enabled {
        return;
    }
    let rungs = |entity: Entity| -> Option<Rungs> {
        let (_, ladder, spacing) = ladders.get(entity).ok()?;
        let spacing = spacing.map(|s| s.0).unwrap_or(0.0);
        Some(Rungs {
            ladder: *ladder,
            spacing: if spacing <= 0.0 {
                DEFAULT_RUNG_SPACING
            } else {
                spacing
            },
        })
    };
    for (e, c, mut intent, mut cc, mut tr, mut s, cfg, children) in &mut q {
        let cc = &mut *cc;
        let s = &mut *s;
        let tr = &mut *tr;
        s.impulse = false;
        let (slide, roll, jump, crouch) = (intent.slide, intent.roll, intent.jump, intent.crouch);
        intent.slide = false;
        intent.roll = false;
        s.guard = (s.guard - dt).max(0.0);
        s.detach = (s.detach - dt).max(0.0);
        s.kick = (s.kick - dt).max(0.0);
        if cc.grounded {
            s.kick = 0.0;
        }
        s.land = (s.land - dt).max(0.0);
        s.queued = (s.queued - dt).max(0.0);
        if s.bounced && cc.grounded {
            s.land = cfg.land_delay;
            if intent.run && intent.move_dir != Vec3::ZERO {
                s.land = 0.0; // still sprinting: it runs straight on
            }
        }
        if cc.grounded || s.active() {
            s.bounced = false;
        }
        s.hint.clear();
        let mut body = children.and_then(|children| {
            children
                .iter()
                .find(|child| bodies.contains(*child))
                .and_then(|child| bodies.get_mut(child).ok())
        });
        let mut facing = Vec3::Z;
        if let Some((_, body_tr)) = &body {
            facing = body_tr.rotation * facing;
        }
        let mut dir = direction(horizontal(intent.move_dir));
        if dir == Vec3::ZERO {
            dir = direction(facing);
        }
        if s.wall_locked
            && (cc.grounded
                || (tr.translation - s.wall_point).dot(s.wall_normal)
                    > cc.radius + cfg.wall_reach + 0.1)
        {
            s.wall_locked = false;
        }
        let acting = body.as_ref().is_some_and(|(st, _)| st.current.one_shot());
        // Stay low while C is held, and until the full capsule fits; never
        // grow through an overhead obstacle.
        if s.mode == Anim::Crouch {
            if !crouch && p.resize_character(e, cc, tr, CAPSULE_HEIGHT) {
                stand_up(s, cc);
            } else {
                cc.walk = direction(intent.move_dir) * 0.8;
                intent.jump = false;
                if !crouch {
                    s.hint = "Move out to stand".into();
                }
                continue;
            }
        }
        if !s.active()
            && cc.height < CAPSULE_HEIGHT
            && !p.resize_character(e, cc, tr, CAPSULE_HEIGHT)
        {
            s.mode = Anim::Crouch;
            cc.walk = direction(intent.move_dir) * 0.8;
            intent.jump = false;
            continue;
        }
        // Holding C crouches, on the ground and not in the middle of
        // something.
        if crouch
            && !s.active()
            && cc.grounded
            && !acting
            && p.resize_character(e, cc, tr, cfg.low_height)
        {
            s.mode = Anim::Crouch;
            cc.walk = direction(intent.move_dir) * 0.8;
            intent.jump = false;
            continue;
        }
        if s.active() {
            intent.jump = false;
            s.elapsed += dt;
            match s.mode {
                Anim::StandUp => {
                    if roll
                        && cc.grounded
                        && s.guard == 0.0
                        && p.resize_character(e, cc, tr, cfg.low_height)
                    {
                        s.mode = Anim::Roll;
                        s.elapsed = 0.0;
                        s.duration = cfg.roll_time;
                        s.direction = dir;
                        s.speed = cfg.roll_distance / cfg.roll_time;
                        s.guard = cfg.guard;
                        cc.walk = Vec3::ZERO;
                        continue;
                    }
                    // Recover while moving, without a stationary lock after
                    // every slide.
                    cc.walk = direction(intent.move_dir)
                        * (c.walk_speed * ease_traversal(s.elapsed / s.duration));
                    if jump && cc.grounded {
                        let launch = cc.walk;
                        end_traversal(s, cc);
                        cc.walk = launch;
                        cc.velocity.y = c.jump_speed;
                        s.impulse = true;
                        if let Some((st, _)) = &mut body {
                            refresh_launch(st, cc, dir);
                        }
                        continue;
                    }
                    if s.elapsed >= s.duration {
                        let walk = cc.walk;
                        end_traversal(s, cc);
                        cc.walk = walk;
                    }
                }
                Anim::LadderEnter | Anim::LadderExit => {
                    cc.controlled = true;
                    let mut next = s.start.lerp(s.end, ease_traversal(s.elapsed / s.duration));
                    if s.mode == Anim::LadderExit {
                        next = ladder_exit_position(s, s.elapsed / s.duration);
                    }
                    if !clear_path(&p, e, cc, tr.translation, next) {
                        end_traversal(s, cc);
                        cc.velocity = Vec3::ZERO;
                        s.detach = 0.5;
                        continue;
                    }
                    cc.walk = (next - tr.translation) / dt;
                    if s.elapsed >= s.duration {
                        tr.translation = s.end;
                        cc.walk = Vec3::ZERO;
                        cc.velocity = Vec3::ZERO;
                        if s.mode == Anim::LadderEnter {
                            s.mode = Anim::LadderClimb;
                            s.elapsed = 0.0;
                        } else {
                            end_traversal(s, cc);
                            s.detach = 0.5;
                            if let Some((st, _)) = &mut body {
                                st.air = 0.0;
                                st.aloft = false;
                            }
                        }
                    }
                }
                Anim::Slide | Anim::Roll => {
                    let slide_mode = s.mode == Anim::Slide;
                    if !cc.grounded {
                        end_traversal(s, cc);
                        continue;
                    }
                    if s.mode == Anim::Slide && jump {
                        let launch = horizontal(cc.walk);
                        cc.velocity.y = c.jump_speed;
                        s.impulse = true;
                        end_traversal(s, cc);
                        cc.walk = launch;
                        if let Some((st, _)) = &mut body {
                            refresh_launch(st, cc, s.direction);
                        }
                        s.guard = cfg.guard;
                        continue;
                    }
                    let mut speed = s.speed;
                    if s.mode == Anim::Roll {
                        // It dives forward fastest, a third of the way in
                        // while stretched out in the air, and slows through
                        // the tuck and getting up. Over the roll it averages
                        // Speed.
                        let u = clamp(s.elapsed / s.duration, 0.0, 1.0);
                        speed *= 12.0 * u * (1.0 - u) * (1.0 - u);
                    }
                    let sprinting = intent.run && intent.move_dir != Vec3::ZERO;
                    if s.mode == Anim::Slide {
                        // Sprinting, it slows to a run to run on at;
                        // otherwise to a stop.
                        let floor = if sprinting {
                            c.run_speed.min(s.speed)
                        } else {
                            0.0
                        };
                        speed = floor + (s.speed - floor) * (1.0 - s.elapsed / s.duration).max(0.0);
                        s.direction = steer_slide(s.direction, s.heading, intent.move_dir, dt);
                    }
                    // Reserve the pose's leading foot/shoulder as well as
                    // capsule travel, so a fast burst stops before its limbs
                    // reach a wall.
                    let clearance = if slide_mode { 0.55 } else { 0.45 };
                    let delta = s.direction * (speed * dt + clearance);
                    if p.sweep_capsule_excluding(tr.translation, delta, cc.radius, cc.height, e)
                        .is_some()
                    {
                        end_traversal(s, cc);
                    } else {
                        cc.walk = s.direction * speed;
                    }
                    let release = if slide_mode {
                        SLIDE_RELEASE
                    } else {
                        ROLL_RELEASE
                    };
                    if s.active()
                        && sprinting
                        && s.elapsed >= release * s.duration
                        && p.resize_character(e, cc, tr, CAPSULE_HEIGHT)
                    {
                        // Still sprinting: up out of the roll or slide into
                        // a run, at the speed it's going, instead of
                        // standing up first.
                        let walk = cc.walk;
                        end_traversal(s, cc);
                        cc.walk = walk;
                    } else if s.elapsed >= s.duration || !s.active() {
                        if p.resize_character(e, cc, tr, CAPSULE_HEIGHT) {
                            end_traversal(s, cc);
                        } else {
                            s.mode = Anim::Crouch;
                            cc.walk = Vec3::ZERO;
                        }
                    }
                }
                Anim::LadderClimb => {
                    s.hint = "W/S climb · Space jump off".into();
                    let Some(ladder) = s.ladder.and_then(rungs) else {
                        end_traversal(s, cc);
                        continue;
                    };
                    let l = ladder.ladder;
                    if jump {
                        end_traversal(s, cc);
                        s.detach = 0.4;
                        cc.walk = l.facing * -cfg.kick_speed;
                        cc.velocity.y = cfg.kick_up;
                        s.impulse = true;
                        if let Some((st, _)) = &mut body {
                            refresh_launch(st, cc, -l.facing);
                        }
                        continue;
                    }
                    let axis = ladder.axis();
                    let mut v = -intent.move_dir.z * cfg.ladder_speed;
                    // Ease acceleration, but releasing the key holds a rung
                    // immediately.
                    if v != 0.0 {
                        v = cc.walk.y + clamp(v - cc.walk.y, -dt * 4.5, dt * 4.5);
                    }
                    cc.controlled = true;
                    cc.walk = axis * v;
                    let feet = tr.translation - Vec3::Y * (cc.height / 2.0);
                    let along = (feet - l.bottom).dot(axis);
                    let spacing = ladder.spacing;
                    s.rung_spacing = spacing;
                    s.phase = (along - LADDER_FOOT) / (2.0 * spacing);
                    let length = ladder.length();
                    let mut exit = l.bottom_exit;
                    // Climb off while the hands still have rail to hold:
                    // from where climbing on by LADDER_EXIT_RISE leaves the
                    // hop to the landing.
                    let top = along
                        >= (length + 0.02 - LADDER_EXIT_HOP - LADDER_EXIT_RISE).max(0.0)
                        && v > 0.0;
                    let leave = (along <= 0.02 && v < 0.0) || top;
                    if top {
                        exit = l.top_exit;
                    }
                    if leave {
                        let target = exit + Vec3::Y * (cc.height / 2.0 + 0.02);
                        let mut route = Traversal {
                            mode: Anim::LadderExit,
                            start: tr.translation,
                            end: target,
                            duration: LADDER_EXIT_TIME,
                            direction: l.facing,
                            exit_top: top,
                            ladder: s.ladder,
                            ..Default::default()
                        };
                        route.lift = Vec3::new(route.start.x, target.y, route.start.z);
                        if !top {
                            route.duration = 0.35;
                        }
                        if clear_path(&p, e, cc, route.start, route.lift)
                            && clear_path(&p, e, cc, route.lift, route.end)
                        {
                            *s = route;
                            cc.walk = Vec3::ZERO;
                            cc.velocity = Vec3::ZERO;
                        } else {
                            cc.walk = Vec3::ZERO;
                            s.hint = "Exit blocked · Space jump off".into();
                        }
                    }
                }
                Anim::Vault | Anim::Mantle => {
                    cc.controlled = true;
                    let next = path_position(s, (s.elapsed / s.duration).min(1.0));
                    if !clear_path(&p, e, cc, tr.translation, next) {
                        end_traversal(s, cc);
                        cc.velocity = Vec3::ZERO;
                        continue;
                    }
                    cc.walk = (next - tr.translation) / dt;
                    if s.elapsed >= s.duration {
                        tr.translation = s.end;
                        end_traversal(s, cc);
                        cc.velocity = Vec3::ZERO;
                    }
                }
                _ => {}
            }
            if !s.active() {
                if cc.height < CAPSULE_HEIGHT && !p.resize_character(e, cc, tr, CAPSULE_HEIGHT) {
                    s.mode = Anim::Crouch;
                }
                if let Some((st, _)) = &mut body {
                    st.launch = horizontal(cc.walk);
                    st.aloft = false;
                }
            }
            if let Some((_, body_tr)) = &mut body
                && s.direction != Vec3::ZERO
            {
                let target = Quat::from_axis_angle(Vec3::Y, s.direction.x.atan2(s.direction.z));
                body_tr.rotation = body_tr.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
            continue;
        }
        if acting {
            continue;
        }
        // A ladder attaches only on deliberate movement toward its face.
        if s.detach == 0.0 && intent.move_dir != Vec3::ZERO {
            for (le, l, spacing) in &ladders {
                if s.active() || direction(intent.move_dir).dot(l.facing) < 0.5 {
                    continue;
                }
                let feet = tr.translation - Vec3::Y * (cc.height / 2.0);
                let axis = direction(l.top - l.bottom);
                let along = (feet - l.bottom).dot(axis);
                let length = l.bottom.distance(l.top);
                let anchor = l.bottom + axis * clamp(along, 0.0, length);
                // Stand in front of the rungs, within reach of them.
                let offset = feet - anchor;
                let ahead = offset.dot(l.facing);
                let beside = (offset - l.facing * ahead).length();
                if along < -0.2
                    || along > length + 0.2
                    || ahead.abs() > cfg.ladder_reach
                    || beside > (l.width / 2.0).max(cfg.ladder_reach)
                {
                    continue;
                }
                let target = anchor + Vec3::Y * (cc.height / 2.0 + 0.02);
                if !clear_path(&p, e, cc, tr.translation, target) {
                    continue;
                }
                s.start = tr.translation;
                s.end = target;
                s.mode = Anim::LadderEnter;
                s.elapsed = 0.0;
                s.duration = 0.35;
                s.ladder = Some(le);
                s.rung_spacing = spacing.map(|r| r.0).unwrap_or(0.0);
                if s.rung_spacing <= 0.0 {
                    s.rung_spacing = DEFAULT_RUNG_SPACING;
                }
                s.phase = (along - LADDER_FOOT) / (2.0 * s.rung_spacing);
                s.direction = l.facing;
                cc.controlled = true;
                cc.walk = Vec3::ZERO;
                cc.velocity = Vec3::ZERO;
                intent.jump = false;
            }
            if s.active() {
                continue;
            }
        }
        if s.guard == 0.0
            && cc.grounded
            && (roll || (slide && intent.run && horizontal(cc.velocity).length() >= cfg.slide_min))
        {
            if !p.resize_character(e, cc, tr, cfg.low_height) {
                continue;
            }
            s.mode = Anim::Roll;
            s.duration = cfg.roll_time;
            s.speed = cfg.roll_distance / cfg.roll_time;
            if !roll {
                s.mode = Anim::Slide;
                s.duration = cfg.slide_time;
                s.speed = cfg.slide_speed.max(horizontal(cc.velocity).length());
                // The way it's running, whatever it wants: it can't slide
                // off the other way.
                dir = direction(horizontal(cc.velocity));
                s.heading = dir.x.atan2(dir.z);
            }
            s.direction = dir;
            s.elapsed = 0.0;
            s.guard = cfg.guard;
            cc.walk = dir * s.speed;
            if s.mode == Anim::Roll {
                cc.walk = Vec3::ZERO;
            }
            intent.jump = false;
            continue;
        }
        if intent.move_dir != Vec3::ZERO {
            let mut route = None;
            if !cc.grounded {
                // In the air it can't line itself up, so heading into a
                // wall at an angle (a wall kick's, say) it climbs square on
                // to it.
                if let Some(hit) =
                    p.cast_ray_excluding(tr.translation, dir, cc.radius + 2.0 * cfg.wall_reach, e)
                    && hit.normal.y.abs() <= 0.2
                {
                    let into = direction(horizontal(-hit.normal));
                    if dir.dot(into) >= AIR_CLIMB_ANGLE {
                        route = obstacle_route(&p, e, cc, tr.translation, into, cfg.wall_reach);
                    }
                }
            }
            if route.is_none() {
                route = obstacle_route(&p, e, cc, tr.translation, dir, cfg.reach);
            }
            if let Some(route) = route {
                s.hint = format!("Space · {}", route.mode);
                // On the ground it turns to face the obstacle before it goes
                // over (see face), rather than spinning round in the middle
                // of the vault: asked while it faces elsewhere, it goes once
                // it's turned, if that's soon. In the air it goes at once.
                if jump {
                    s.queued = OBSTACLE_WAIT;
                    intent.jump = false;
                }
                let facing_it = !cc.grounded || facing.dot(route.direction) >= OBSTACLE_FACING;
                if s.queued > 0.0 && s.guard == 0.0 && facing_it {
                    *s = route;
                    s.guard = cfg.guard;
                    cc.controlled = true;
                    cc.walk = Vec3::ZERO;
                    cc.velocity = Vec3::ZERO;
                    intent.jump = false;
                    continue;
                }
            }
        }
        // A wall kick takes the speed of a sprint's jump, or of another
        // kick.
        let v = horizontal(cc.walk);
        if !cc.grounded && (s.bounced || v.length() >= cfg.kick_min) {
            let dirs = [dir, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
            for d in dirs {
                let Some(hit) =
                    p.cast_ray_excluding(tr.translation, d, cc.radius + cfg.wall_reach, e)
                else {
                    continue;
                };
                let wall = p.body_of(hit.entity);
                if !p.static_surface(hit.entity)
                    || hit.normal.y.abs() > 0.2
                    || (s.wall_locked
                        && s.last_wall == Some(wall)
                        && hit.normal.dot(s.wall_normal) > 0.9)
                {
                    continue;
                }
                s.hint = "Space · wall kick".into();
                if !jump || s.guard > 0.0 {
                    break;
                }
                let n = direction(horizontal(hit.normal));
                // It comes off the wall like a ball: the way it came in,
                // reflected, and at least a little outwards.
                let mut out = direction(v);
                let tangent = out - n * out.dot(n);
                out = direction(tangent + n * out.dot(n).abs().max(0.35));
                cc.walk = out * cfg.kick_speed;
                s.bounced = true;
                cc.velocity.y = cfg.kick_up;
                s.impulse = true;
                s.last_wall = Some(wall);
                s.wall_normal = n;
                s.wall_point = hit.point;
                s.wall_locked = true;
                s.guard = cfg.guard;
                s.kick = WALL_KICK_TIME;
                // The foot nearer the wall taps it: the left one with the
                // wall on the left of the way the body is going.
                let mut along = tangent;
                if along.length_squared() < 0.01 {
                    along = facing;
                }
                s.kick_right = along.dot(Vec3::new(n.z, 0.0, -n.x)) < 0.0;
                intent.jump = false;
                if let Some((st, _)) = &mut body {
                    let launch = direction(cc.walk);
                    refresh_launch(st, cc, launch);
                }
                break;
            }
        }
    }
}

/// A vault or a mantle over whatever is ahead along `dir` within `reach`,
/// if there's one to be had.
pub fn obstacle_route(
    p: &CharacterPhysics,
    e: Entity,
    cc: &CharacterController,
    center: Vec3,
    dir: Vec3,
    reach: f32,
) -> Option<Traversal> {
    let feet = center.y - cc.height / 2.0;
    let hit = p.cast_ray_excluding(
        Vec3::new(center.x, feet + 0.3, center.z),
        dir,
        cc.radius + reach,
        e,
    )?;
    if !p.static_surface(hit.entity) || hit.normal.y.abs() > 0.2 {
        return None;
    }
    let wall = p.body_of(hit.entity);
    let same = |other: &RayHit| p.body_of(other.entity) == wall;
    let mut near = hit.point + dir * 0.05;
    near.y = feet + 1.9;
    let top = p.cast_ray_excluding(near, Vec3::NEG_Y, 1.9, e)?;
    if !same(&top) || top.normal.y < 0.7 {
        return None;
    }
    let height = top.point.y - feet;
    if !(0.35..=1.8).contains(&height) {
        return None;
    }
    let mut route = Traversal {
        mode: Anim::Mantle,
        duration: 0.8,
        start: center,
        direction: dir,
        ..Default::default()
    };
    route.end = hit.point + dir * (cc.radius + 0.08);
    route.end.y = top.point.y + cc.height / 2.0 + 0.04;
    if height <= 1.0 {
        // Locate the far edge within the allowed depth, then require a safe
        // landing.
        let mut depth = 0.15;
        while depth <= 1.05 {
            let mut probe = hit.point + dir * depth;
            probe.y = top.point.y + 0.1;
            if p.cast_ray_excluding(probe, Vec3::NEG_Y, 0.25, e)
                .is_some_and(|floor| same(&floor))
            {
                depth += 0.1;
                continue;
            }
            let mut landing = probe + dir * (cc.radius + 0.08);
            landing.y = top.point.y + 0.1;
            if let Some(ground) = p.cast_ray_excluding(landing, Vec3::NEG_Y, height + 0.3, e)
                && ground.normal.y >= 0.7
            {
                route.mode = Anim::Vault;
                route.duration = 1.0;
                route.end = Vec3::new(
                    landing.x,
                    ground.point.y + cc.height / 2.0 + 0.04,
                    landing.z,
                );
            }
            break;
        }
    }
    if route.mode == Anim::Mantle {
        let mut probe = route.end;
        probe.y = top.point.y + 0.1;
        let support = p.cast_ray_excluding(probe, Vec3::NEG_Y, 0.2, e)?;
        if !same(&support) || support.normal.y < 0.7 {
            return None;
        }
    }
    route.lift = Vec3::new(center.x, top.point.y + cc.height / 2.0 + 0.08, center.z);
    let across = Vec3::new(route.end.x, route.lift.y, route.end.z);
    if !clear_path(p, e, cc, center, route.lift)
        || !clear_path(p, e, cc, route.lift, across)
        || !clear_path(p, e, cc, across, route.end)
    {
        return None;
    }
    Some(route)
}
