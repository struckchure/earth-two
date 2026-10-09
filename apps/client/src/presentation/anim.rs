//! What a body does, and the rules that pick it: character/character.go's
//! Anim and the pure parts of character/animate.go.

use bevy::math::Vec3;

pub use crate::character::{Anim, Character};

/// wall_kicking reports whether a is the wall kick, the fall after it, or
/// one of their mirror images (WallKick to WallFallRight, in order).
pub fn wall_kicking(a: Anim) -> bool {
    a >= Anim::WallKick && a <= Anim::WallFallRight
}

/// Clip is the animation an Anim plays.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    pub name: String,
    /// Start is how many seconds into the clip to begin. Jumps start at
    /// take-off, since the physics jump happens at once.
    pub start: f32,
    /// Land, for jumps, is when the feet touch down. The clip plays slower or
    /// faster so take-off to Land lasts as long as the jump, and holds at
    /// Land while the character is still falling.
    pub land: f32,
    /// Hold, for jumps, keeps the clip at Start for as long as the character
    /// is airborne: a pose, for a model without a jump clip.
    pub hold: bool,
    /// Loop, for jumps, loops the clip for as long as the character is
    /// airborne: an in-air cycle.
    pub r#loop: bool,
    /// Step, for stairs, is the share of the clip's cycle at which a foot
    /// is planted on a step at the height of the floor the stairs stand on.
    pub step: f32,
    /// Backward plays the clip backwards: coming down stairs with the clip
    /// for climbing them, for a body with no clip of its own for that.
    pub backward: bool,
}

impl Clip {
    pub fn named(name: &str) -> Clip {
        Clip {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// MOVING is the speed below which a character counts as standing.
pub const MOVING: f32 = 0.3;
/// COYOTE is how long a character can be off the ground, stepping down a
/// curb say, before it looks airborne.
pub const COYOTE: f32 = 0.12;
pub const FADE: f32 = 0.2;
/// RELEASE is the share of an action's clip after which a character that
/// wants to move stops acting and gets going, skipping the recovery back to
/// standing; RESUME is how long, leaving one, it counts as walking or running
/// while it gets back up to speed.
pub const RELEASE: f32 = 0.7;
pub const RESUME: f32 = 0.3;
/// PIVOT is how fast (radians per second) a body turns before it steps round
/// rather than swivelling on the spot.
pub const PIVOT: f32 = 1.0;
/// STAIR_CYCLE is how far up a stair clip's cycle climbs: two steps, a foot
/// on each. The kit's stairs are twelve 0.3 m steps up a 3.6 m deck, so a
/// deck is six whole cycles.
pub const STAIR_CYCLE: f32 = 0.6;
/// A slope counts as stairs from STAIRS_FROM to STAIRS_TO off level (the
/// stairs' ramp is 33°), and it stays stairs for STAIRS_HOLD after.
pub const STAIRS_FROM: f32 = 18.0 * std::f32::consts::PI / 180.0;
pub const STAIRS_TO: f32 = 50.0 * std::f32::consts::PI / 180.0;
pub const STAIRS_HOLD: f32 = 0.15;
/// CROUCH_STRIDE is how far a crouched walk's cycle goes, two steps: 1.2 m
/// on the man, 1.05 m on the woman.
pub const CROUCH_STRIDE: f32 = 1.13;
/// FALL_FADE is how long the fall's leg swing takes to fade in out of the
/// jump's pose.
pub const FALL_FADE: f32 = 0.3;
/// ROLL_RUN is the share of the run clip a roll or slide runs on into: the
/// left foot planted ahead and the right behind, as both leave them.
pub const ROLL_RUN: f32 = 0.25;
/// WALL_KICK_TIME matches the wall kick clip: one foot taps the wall and
/// pushes off (character/traversal.go).
pub const WALL_KICK_TIME: f32 = 8.0 / 30.0;
/// ROLL_RELEASE is the share of a roll after which a sprint runs on
/// (character/traversal.go).
pub const ROLL_RELEASE: f32 = 0.6;

/// ROLL_KEYS retimes the roll's clip: pairs of how far through the roll (0
/// to 1) and how far through its clip. It lingers on the dive, the body
/// stretched out forward in the air, and hurries through the tuck, so it
/// reads as a dive and roll rather than a ball.
const ROLL_KEYS: [[f32; 2]; 5] = [
    [0.0, 0.0],
    [0.1, 0.09],
    [0.4, 0.3],
    [0.55, 0.55],
    [1.0, 1.0],
];

/// roll_clip is how far through the roll's clip to show u of the way through
/// the roll.
pub fn roll_clip(u: f32) -> f32 {
    let u = clamp(u, 0.0, 1.0);
    for i in 1..ROLL_KEYS.len() {
        let (a, b) = (ROLL_KEYS[i - 1], ROLL_KEYS[i]);
        if u <= b[0] {
            return a[1] + (b[1] - a[1]) * (u - a[0]) / (b[0] - a[0]);
        }
    }
    1.0
}

/// Motion is what the animation state machine knows about a character.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    pub grounded: bool,
    /// Horizontal.
    pub speed: f32,
    /// Resuming is set just after an action while the character wants to
    /// move on, and pivoting while it turns about to head somewhere, too
    /// slow to count as moving: both step, rather than stand.
    pub resuming: bool,
    pub pivoting: bool,
}

/// pick_anim chooses what a body should play. acting reports whether the
/// current one-shot is still playing.
pub fn pick_anim(m: Motion, c: &Character, current: Anim, acting: bool) -> Anim {
    if !m.grounded {
        if current.airborne() {
            return current; // a jump plays through, whatever the speed does
        }
        if m.speed > MOVING {
            return Anim::RunJump;
        }
        return Anim::Jump;
    }
    if current.one_shot() && acting {
        return current; // it stands still for it; only a fall cuts it short
    }
    if m.speed > MOVING {
        if m.speed > (c.walk_speed + c.run_speed) / 2.0 {
            return Anim::Run;
        }
        return Anim::Walk;
    }
    if m.resuming || m.pivoting {
        // Straight from the action into a walk as it speeds back up (into a
        // run, if it's running, once it's going fast enough); or stepping
        // round on the spot.
        return Anim::Walk;
    }
    Anim::Idle
}

/// on_stairs is which way a character standing on ground with normal n and
/// going at velocity v is going on stairs: 1 up, -1 down, 0 not on any (on
/// the level, too steep, or not going anywhere).
pub fn on_stairs(n: Vec3, v: Vec3) -> i8 {
    let slope = clamp(n.y, -1.0, 1.0).acos();
    if slope < STAIRS_FROM || slope > STAIRS_TO || v.x.hypot(v.z) < MOVING {
        return 0;
    }
    // The normal leans downhill.
    if v.x * n.x + v.z * n.z < 0.0 {
        return 1;
    }
    -1
}

/// stairs_anim is the clip for going dir on stairs (1 up, -1 down), or Idle
/// for none.
pub fn stairs_anim(dir: i8) -> Anim {
    match dir {
        1 => Anim::StairsUp,
        -1 => Anim::StairsDown,
        _ => Anim::Idle,
    }
}

/// stair_phase is how far through its cycle a stair clip is, 0 to 1, with
/// the feet at height y, going up or down: a cycle a STAIR_CYCLE, a foot
/// planted at step's share of it on each step's edge.
pub fn stair_phase(y: f32, up: bool, step: f32) -> f32 {
    let mut climbed = y / STAIR_CYCLE;
    if !up {
        climbed = -climbed;
    }
    let p = climbed + step;
    p - p.floor()
}

/// can_act reports whether a character can start an action: on the ground,
/// standing or on the move, and not in the middle of one already.
pub fn can_act(m: Motion, acting: bool) -> bool {
    m.grounded && !acting
}

/// air_speed is the playback speed that stretches a jump clip's take-off to
/// landing over the airtime of a jump at jump_speed.
pub fn air_speed(clip: &Clip, jump_speed: f32, gravity: f32) -> f32 {
    if clip.land <= clip.start || jump_speed <= 0.0 || gravity <= 0.0 {
        return 1.0;
    }
    let airtime = 2.0 * jump_speed / gravity;
    clamp((clip.land - clip.start) / airtime, 0.25, 2.0)
}

pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    lo.max(v.min(hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    // character/animate_test.go TestPickAnim.
    #[test]
    fn pick_anim_table() {
        let c = Character::default(); // walk 1.6, run 4.6: runs above 3.1
        let g = |speed| Motion {
            grounded: true,
            speed,
            ..Default::default()
        };
        let a = |speed| Motion {
            speed,
            ..Default::default()
        };
        let cases: &[(&str, Motion, Anim, bool, Anim)] = &[
            ("standing", g(0.0), Anim::Idle, false, Anim::Idle),
            ("barely drifting", g(0.2), Anim::Idle, false, Anim::Idle),
            ("walking", g(1.6), Anim::Idle, false, Anim::Walk),
            (
                "jogging stays a walk",
                g(3.0),
                Anim::Walk,
                false,
                Anim::Walk,
            ),
            ("running", g(5.0), Anim::Walk, false, Anim::Run),
            (
                "jumping from standing",
                a(0.0),
                Anim::Idle,
                false,
                Anim::Jump,
            ),
            (
                "jumping on the move",
                a(5.0),
                Anim::Run,
                false,
                Anim::RunJump,
            ),
            (
                "a standing jump stays one when steered",
                a(5.0),
                Anim::Jump,
                false,
                Anim::Jump,
            ),
            (
                "a running jump stays one when stopped",
                a(0.0),
                Anim::RunJump,
                false,
                Anim::RunJump,
            ),
            ("landing still", g(0.0), Anim::Jump, false, Anim::Idle),
            (
                "landing on the move",
                g(5.0),
                Anim::RunJump,
                false,
                Anim::Run,
            ),
            ("acting", g(0.0), Anim::Punch, true, Anim::Punch),
            ("action done", g(0.0), Anim::Punch, false, Anim::Idle),
            (
                "a shove doesn't cut an action short",
                g(2.0),
                Anim::Interact,
                true,
                Anim::Interact,
            ),
            (
                "moving once it's done",
                g(2.0),
                Anim::Interact,
                false,
                Anim::Walk,
            ),
            (
                "walking on out of an action",
                Motion {
                    grounded: true,
                    resuming: true,
                    ..Default::default()
                },
                Anim::Punch,
                false,
                Anim::Walk,
            ),
            (
                "running on out of an action starts with a walk",
                Motion {
                    grounded: true,
                    resuming: true,
                    ..Default::default()
                },
                Anim::PickUp,
                false,
                Anim::Walk,
            ),
            (
                "still walking while speeding up",
                Motion {
                    grounded: true,
                    speed: 0.1,
                    resuming: true,
                    ..Default::default()
                },
                Anim::Walk,
                false,
                Anim::Walk,
            ),
            (
                "a finished action with nowhere to go",
                g(0.0),
                Anim::Punch,
                false,
                Anim::Idle,
            ),
            (
                "stepping round turning about",
                Motion {
                    grounded: true,
                    speed: 0.1,
                    pivoting: true,
                    ..Default::default()
                },
                Anim::Walk,
                false,
                Anim::Walk,
            ),
            (
                "stepping round from standing",
                Motion {
                    grounded: true,
                    pivoting: true,
                    ..Default::default()
                },
                Anim::Idle,
                false,
                Anim::Walk,
            ),
            (
                "falling cuts an action short",
                a(0.0),
                Anim::PickUp,
                true,
                Anim::Jump,
            ),
            (
                "a fall plays on in the air",
                a(5.0),
                Anim::Fall,
                false,
                Anim::Fall,
            ),
            ("landing from a fall", g(0.0), Anim::Fall, false, Anim::Idle),
            (
                "landing from a fall on the move",
                g(5.0),
                Anim::Fall,
                false,
                Anim::Run,
            ),
        ];
        for (name, m, current, acting, want) in cases {
            assert_eq!(pick_anim(*m, &c, *current, *acting), *want, "{name}");
        }
    }

    // TestAnimKinds.
    #[test]
    fn anim_kinds() {
        for a in [
            Anim::Idle,
            Anim::Walk,
            Anim::Run,
            Anim::Jump,
            Anim::RunJump,
            Anim::Interact,
            Anim::Punch,
            Anim::PunchRight,
            Anim::PickUp,
        ] {
            assert!(
                !(a.airborne() && a.one_shot()),
                "{a} is both airborne and a one-shot"
            );
        }
        assert!(wall_kicking(Anim::WallFallRight) && !wall_kicking(Anim::WallLand));
        for a in [Anim::Interact, Anim::Punch, Anim::PunchRight, Anim::PickUp] {
            assert!(a.one_shot(), "{a} isn't a one-shot");
        }
        for a in [Anim::Jump, Anim::RunJump, Anim::Fall] {
            assert!(a.airborne(), "{a} isn't airborne");
        }
    }

    // TestRollClip.
    #[test]
    fn roll_clip_runs_forward() {
        assert_eq!((roll_clip(0.0), roll_clip(1.0)), (0.0, 1.0));
        let mut u = 0.0;
        while u < 1.0 {
            assert!(
                roll_clip(u + 0.01) >= roll_clip(u),
                "roll clip goes back at {u}"
            );
            u += 0.01;
        }
        // A sprinting roll runs on with its feet planted.
        let c = roll_clip(ROLL_RELEASE);
        assert!(
            (0.58..=0.65).contains(&c),
            "roll runs on at {c} of its clip"
        );
    }

    // TestAirSpeed.
    #[test]
    fn air_speed_stretches_the_jump() {
        let jump = |start, land| Clip {
            start,
            land,
            ..Default::default()
        };
        let got = air_speed(&jump(0.3, 0.7), 4.0, 10.0);
        assert!((got - 0.5).abs() < 1e-4, "airSpeed = {got}, want 0.5");
        assert_eq!(air_speed(&jump(0.3, 0.0), 4.0, 10.0), 1.0);
        assert_eq!(air_speed(&jump(0.0, 0.01), 4.0, 10.0), 0.25);
    }

    // TestOnStairs.
    #[test]
    fn on_stairs_table() {
        let ramp = Vec3::new(0.0, 0.84, 0.54).normalize();
        let cases: &[(&str, Vec3, Vec3, i8)] = &[
            ("climbing", ramp, Vec3::new(0.0, 0.0, -1.6), 1),
            ("coming down", ramp, Vec3::new(0.0, 0.0, 1.6), -1),
            ("standing on them", ramp, Vec3::ZERO, 0),
            ("on the level", Vec3::Y, Vec3::new(0.0, 0.0, -1.6), 0),
            (
                "a gentle ramp",
                Vec3::new(0.0, 1.0, 0.15).normalize(),
                Vec3::new(0.0, 0.0, -1.6),
                0,
            ),
            ("a wall", Vec3::Z, Vec3::new(0.0, 0.0, -1.6), 0),
        ];
        for (name, n, v, want) in cases {
            assert_eq!(on_stairs(*n, *v), *want, "{name}");
        }
    }

    // TestStairPhase.
    #[test]
    fn stair_phase_plants_feet() {
        for y in [0.0, STAIR_CYCLE, 3.6] {
            let got = stair_phase(y, true, 0.25);
            assert!((got - 0.25).abs() <= 1e-4, "climbing at {y}: phase {got}");
        }
        let got = stair_phase(STAIR_CYCLE / 2.0, true, 0.0);
        assert!((got - 0.5).abs() <= 1e-4);
        let (up, down) = (stair_phase(0.1, true, 0.0), stair_phase(0.1, false, 0.0));
        assert!((up + down - 1.0).abs() <= 1e-4);
    }
}
