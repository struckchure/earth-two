//! The animation state machine: character/animate.go's animate system.

use avian3d::prelude::{Gravity, Physics};
use bevy::prelude::*;

use super::anim::*;
use super::paused;
use super::player::{AnimationPlayer, ClipLibrary, Libraries};
use super::presentation::MotionSamples;
use super::roster::{Roster, Skin, action};
use crate::character::{Body, Character, CharacterController, Controls, Intent, State, Traversal};

/// released reports whether p, playing an action on skin, is past RELEASE:
/// far enough through that a character wanting to move can move on.
fn released(lib: &ClipLibrary, p: &AnimationPlayer) -> bool {
    lib.duration_of(p.clip())
        .is_some_and(|d| p.time() >= RELEASE * d)
}

/// traversal_time matches the same previous/current interval used by the
/// interpolated body.
fn traversal_time(s: &Traversal, overstep: f32, timestep: f32) -> f32 {
    clamp(s.elapsed + (overstep - 1.0) * timestep, 0.0, s.duration)
}

/// play starts clip, crossfading from whatever was playing. Once-clips hold
/// their last frame; starting the one already playing starts it over.
pub fn play(p: &mut AnimationPlayer, clip: &Clip, once: bool) {
    p.paused = false;
    if !once {
        p.play(&clip.name).fade_in(FADE);
    } else if p.clip() == clip.name {
        p.replay();
    } else {
        p.play_once(&clip.name).fade_in(FADE / 2.0);
    }
    if clip.start > 0.0 {
        p.seek(clip.start);
    }
}

/// animate runs the state machine for every body, playing its skin's clip
/// for its state. It runs in Update, before advance_animations moves the
/// clocks.
#[allow(clippy::too_many_arguments)]
pub fn animate(
    mut bodies: Query<(&ChildOf, &mut State, &mut AnimationPlayer), With<Body>>,
    mut roots: Query<(
        &Character,
        &mut Intent,
        &CharacterController,
        &Traversal,
        &MotionSamples,
    )>,
    roster: Res<Roster>,
    libraries: Res<Libraries>,
    controls: Res<Controls>,
    physics: Option<Res<Time<Physics>>>,
    gravity: Option<Res<Gravity>>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
) {
    let paused = paused(physics.as_deref());
    let gravity = gravity.map_or(9.81, |g| -g.0.y);
    let dt = time.delta_secs();
    let frame = Frame {
        enabled: controls.enabled,
        paused,
        gravity,
        dt,
        overstep: fixed.overstep_fraction(),
        timestep: fixed.timestep().as_secs_f32(),
    };
    for (parent, mut st, mut p) in &mut bodies {
        let Ok((c, mut intent, cc, traversal, samples)) = roots.get_mut(parent.parent()) else {
            continue;
        };
        let Some(skin) = roster.skins.get(st.skin) else {
            continue;
        };
        let lib = libraries.get(&skin.model);
        let mut body = BodyInput {
            character: c,
            intent: &mut intent,
            controller: cc,
            traversal,
            samples,
        };
        animate_body(&frame, skin, lib, &mut st, &mut p, &mut body);
    }
}

/// What a frame of the state machine needs from the clocks and settings.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub enabled: bool,
    pub paused: bool,
    pub gravity: f32,
    pub dt: f32,
    pub overstep: f32,
    pub timestep: f32,
}

/// BodyInput is what one body's frame reads from its root.
pub struct BodyInput<'a> {
    pub character: &'a Character,
    pub intent: &'a mut Intent,
    pub controller: &'a CharacterController,
    pub traversal: &'a Traversal,
    pub samples: &'a MotionSamples,
}

/// animate_body runs one body's frame of the state machine.
pub fn animate_body(
    f: &Frame,
    skin: &Skin,
    lib: Option<&ClipLibrary>,
    st: &mut State,
    p: &mut AnimationPlayer,
    body: &mut BodyInput,
) {
    let traversal = body.traversal;
    let c = body.character;
    let cc = body.controller;
    let samples = body.samples;
    // Menus pause the entire blended pose. Resetting to Idle while CPU cloth
    // holds its last mesh separates the naked body from the garment.
    if !f.enabled || f.paused {
        p.paused = true;
        return;
    }
    p.paused = false;
    let input = &mut *body.intent;
    // The clip clock reads st.turn as the facing system left it this frame.
    p.manual_time = false;
    if !traversal.active() && traversal.kick == 0.0 && !cc.grounded && wall_kicking(st.current) {
        // After a wall kick it falls as the kick left it, loosely, until it
        // lands.
        p.paused = !f.enabled || f.paused;
        if p.paused {
            return;
        }
        let mut next = Anim::WallFall;
        if st.current == Anim::WallKickRight || st.current == Anim::WallFallRight {
            next = Anim::WallFallRight;
        }
        if next != st.current {
            st.current = next;
            play(p, &skin.clip(next), false);
            p.fade_in(0.2);
        }
        input.act = Anim::Idle;
        p.speed = 1.0;
        return;
    }
    if traversal.active() || traversal.kick > 0.0 {
        if !f.enabled || f.paused {
            p.paused = true;
            return;
        }
        p.paused = false;
        p.manual_time = true;
        let mut next = traversal.mode;
        if !traversal.active() {
            next = if traversal.kick_right {
                Anim::WallKickRight
            } else {
                Anim::WallKick
            };
        }
        // Crouched and going somewhere, it walks crouched.
        let speed = samples.speed(f.timestep);
        if next == Anim::Crouch && speed > MOVING && skin.has(Anim::CrouchWalk) {
            next = Anim::CrouchWalk;
        }
        if next != st.current {
            let crouched = st.current == Anim::Crouch || st.current == Anim::CrouchWalk;
            st.current = next;
            play(
                p,
                &skin.clip(next),
                !matches!(
                    next,
                    Anim::LadderClimb | Anim::LadderEnter | Anim::Crouch | Anim::CrouchWalk
                ),
            );
            p.fade_in(0.22);
            if next == Anim::Slide || next == Anim::Roll {
                p.fade_in(0.09);
            }
            if next == Anim::WallKick || next == Anim::WallKickRight {
                p.fade_in(0.07);
            }
            if next == Anim::StandUp {
                p.fade_in(0.08);
                // Its clip starts from a deeper squat than a crouch: blended
                // in slowly, the body doesn't drop into it.
                if crouched {
                    p.fade_in(0.25);
                }
            }
            if next == Anim::LadderExit && !traversal.exit_top {
                p.fade_in(0.12);
            }
            if next == Anim::LadderClimb {
                p.fade_in(0.12);
            }
        }
        input.act = Anim::Idle;
        st.turn = 0.0;
        p.speed = 1.0;
        if let Some(duration) = lib.and_then(|a| a.duration_of(&skin.clip(next).name)) {
            match next {
                Anim::WallKick | Anim::WallKickRight => {
                    let elapsed = WALL_KICK_TIME - traversal.kick + (f.overstep - 1.0) * f.timestep;
                    p.seek(clamp(elapsed / WALL_KICK_TIME, 0.0, 1.0) * duration);
                }
                Anim::LadderClimb => {
                    let spacing = traversal.rung_spacing.max(0.01);
                    let mut phase =
                        traversal.phase + cc.walk.y * f.overstep * f.timestep / (2.0 * spacing);
                    phase -= phase.floor();
                    p.seek(phase * duration);
                }
                Anim::LadderEnter => {
                    let phase = traversal.phase - traversal.phase.floor();
                    p.seek(phase * duration);
                }
                Anim::Crouch => {
                    // Crouched and still: Mixamo's Crouching Idle, on its clock.
                    st.crouch_idle = (st.crouch_idle + f.dt) % duration;
                    p.seek(st.crouch_idle);
                }
                Anim::CrouchWalk => {
                    // A cycle a CROUCH_STRIDE gone, so the feet keep to the ground.
                    st.crouch_phase += speed * f.dt / CROUCH_STRIDE;
                    st.crouch_phase -= st.crouch_phase.floor();
                    p.seek(st.crouch_phase * duration);
                }
                Anim::LadderExit => {
                    if !traversal.exit_top {
                        p.seek(duration);
                    } else {
                        p.seek(
                            traversal_time(traversal, f.overstep, f.timestep) / traversal.duration
                                * duration,
                        );
                    }
                }
                Anim::Roll => {
                    if traversal.duration > 0.0 {
                        p.seek(
                            roll_clip(
                                traversal_time(traversal, f.overstep, f.timestep)
                                    / traversal.duration,
                            ) * duration,
                        );
                    }
                }
                _ => {
                    if traversal.duration > 0.0 && next != Anim::WallKick {
                        p.seek(
                            traversal_time(traversal, f.overstep, f.timestep) / traversal.duration
                                * duration,
                        );
                    }
                }
            }
        }
        return;
    }

    if cc.grounded {
        st.air = 0.0;
    } else {
        st.air += f.dt;
    }
    let mut m = Motion {
        // A jump shows at once; a step down only after a moment.
        grounded: !f.enabled || cc.grounded || st.air < COYOTE && cc.velocity.y <= 0.0,
        speed: cc.velocity.x.hypot(cc.velocity.z),
        ..Default::default()
    };
    // Against a wall it wants to go but doesn't: it stands, rather than
    // walking on the spot. (The lesser of the two, as a teleport is no speed
    // at all.)
    m.speed = m.speed.min(samples.speed(f.timestep));

    let heading = input.move_dir.x != 0.0 || input.move_dir.z != 0.0;
    // Landing from a wall kick it sinks into its knees, unless it's still
    // sprinting: then it runs straight on.
    if wall_kicking(st.current)
        && cc.grounded
        && skin.has(Anim::WallLand)
        && !(input.run && heading)
    {
        st.current = Anim::WallLand;
        play(p, &skin.clip(Anim::WallLand), true);
        p.fade_in(0.08);
    }
    if st.current == Anim::WallLand
        && m.grounded
        && !p.finished()
        && (!heading || traversal.land > 0.0)
    {
        // Taking the landing from a wall kick; wanting to move cuts it
        // short, once it's free to.
        p.speed = 1.0;
        return;
    }
    let mut acting = st.current.one_shot() && !p.finished();
    if acting && heading && lib.is_some_and(|a| released(a, p)) {
        acting = false; // wants to move on: skip the recovery
    }
    if input.act != Anim::Idle {
        // Moving on, a released action is still on screen this frame:
        // starting one now would jump its clip back to the start.
        let showing = st.current.one_shot() && !p.finished();
        let act = action(st, input.act, skin);
        if can_act(m, showing) && skin.has(act) {
            st.current = act;
            acting = true;
            play(p, &skin.clip(act), true);
            if m.speed > MOVING {
                p.fade_in(FADE); // out of a stride: ease into it as it pulls up
            }
            if act == Anim::Punch || act == Anim::PunchRight {
                st.right_punch = act == Anim::Punch;
            }
        }
        input.act = Anim::Idle; // started or not, it's done with
    }
    if st.current.one_shot() && !acting && heading {
        st.resume = RESUME;
    }
    st.resume = (st.resume - f.dt).max(0.0);
    m.resuming = heading && st.resume > 0.0;
    m.pivoting = heading && st.turn.abs() > PIVOT;

    // On stairs, its walk or run climbs (or comes down) them a foot on each
    // step, if it has the clips.
    let dir = on_stairs(cc.ground_normal, cc.velocity);
    if cc.grounded && dir != 0 {
        st.stairs = dir;
        st.stairs_left = STAIRS_HOLD;
    } else {
        st.stairs_left = (st.stairs_left - f.dt).max(0.0);
        if st.stairs_left == 0.0 {
            st.stairs = 0;
        }
    }
    let mut next = pick_anim(m, c, st.current, acting);
    let stairs = stairs_anim(st.stairs);
    if stairs != Anim::Idle && skin.has(stairs) && (next == Anim::Walk || next == Anim::Run) {
        next = stairs;
    }
    // Standing still, a held pose instead of idling, for as long as it's
    // asked for.
    if next == Anim::Idle && input.hold.held() && skin.has(input.hold) {
        next = input.hold;
    }
    if next != st.current {
        let landing = st.current.airborne() && m.grounded;
        let rolled = st.current == Anim::Roll || st.current == Anim::Slide;
        st.current = next;
        let clip = skin.clip(next);
        play(p, &clip, next.airborne() && !clip.r#loop);
        if landing {
            p.fade_in(0.12);
        }
        if rolled && next == Anim::Run {
            // Running on out of a roll or slide: into the stride its feet
            // are in.
            if let Some(d) = lib.and_then(|a| a.duration_of(&clip.name)) {
                p.seek(ROLL_RUN * d);
            }
        }
    }

    // Match the stride to the ground speed, so feet don't slide, and a
    // jump's clip to its airtime.
    match st.current {
        Anim::StairsUp | Anim::StairsDown => {
            // Set by how high the feet are, so a foot lands on each step
            // whatever the pace.
            let clip = skin.clip(st.current);
            if let Some(d) = lib.and_then(|a| a.duration_of(&clip.name)) {
                let feet = samples.position(samples.current(), f.overstep).y - cc.height / 2.0;
                p.manual_time = true;
                p.seek(
                    stair_phase(
                        feet,
                        st.current == Anim::StairsUp || clip.backward,
                        clip.step,
                    ) * d,
                );
            }
        }
        Anim::Walk => p.speed = clamp(m.speed / c.walk_speed, 0.6, 1.6),
        Anim::Run => p.speed = clamp(m.speed / c.run_speed, 0.7, 1.4),
        Anim::Jump | Anim::RunJump => {
            let clip = skin.clip(st.current);
            if clip.hold {
                // Seeking back each frame, rather than pausing, holds the
                // pose and lets the crossfade into it finish.
                p.seek(clip.start);
            } else if clip.r#loop {
                p.speed = 1.0;
            } else {
                p.speed = air_speed(&clip, c.jump_speed, f.gravity);
                if clip.land > 0.0 && p.time() >= clip.land {
                    if skin.has(Anim::Fall) {
                        // Still falling: swing the legs until it lands.
                        st.current = Anim::Fall;
                        play(p, &skin.clip(Anim::Fall), false);
                        p.fade_in(FALL_FADE);
                    } else {
                        p.paused = true; // hold just before touchdown
                    }
                }
            }
        }
        _ => p.speed = 1.0,
    }
}

/// Freeze the last blended animation while the articulated rig supplies its
/// pose. Recovery resets presentation state and returns ownership to clips.
pub fn sync_downed(
    mut bodies: Query<
        (
            Entity,
            &ChildOf,
            &mut State,
            &mut AnimationPlayer,
            &mut Transform,
        ),
        With<Body>,
    >,
    downed: Query<(), With<crate::character::Downed>>,
    mut commands: Commands,
) {
    let _ = &mut commands;
    for (entity, parent, mut state, mut animation, mut tr) in &mut bodies {
        let _ = entity;
        if downed.contains(parent.parent()) {
            state.downed = true;
            state.hidden = false;
            animation.paused = true;
            animation.manual_time = true;
            #[cfg(feature = "viewer")]
            commands.entity(entity).insert(Visibility::Inherited);
        } else if state.downed {
            *state = State {
                skin: state.skin,
                current: state.current,
                ..default()
            };
            animation.paused = false;
            animation.manual_time = false;
            tr.rotation = Quat::IDENTITY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // character/animate_test.go TestPlayStartsAtStart.
    #[test]
    fn play_starts_at_start() {
        let mut p = AnimationPlayer {
            paused: true, // left over from a jump held at Land
            ..default()
        };
        let clip = Clip {
            name: "run".into(),
            start: 0.3,
            hold: true,
            ..Default::default()
        };
        play(&mut p, &clip, true);
        assert!(p.clip() == "run" && !p.paused && p.time() == 0.3);
    }

    // character/skin_test.go TestLoopingJumpDoesNotFinish.
    #[test]
    fn looping_jump_does_not_finish() {
        let mut p = AnimationPlayer::default();
        let clip = Clip {
            name: "jump-loop".into(),
            r#loop: true,
            ..Default::default()
        };
        play(&mut p, &clip, false);
        assert!(p.clip() == "jump-loop" && !p.finished());
    }
}
