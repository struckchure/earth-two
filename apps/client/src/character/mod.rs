//! The people of Earth Two: how they move. Every character shares one body,
//! one locomotion system and one traversal state machine, and differs only
//! in what writes its [`Intent`]: the keyboard for the player, and later,
//! job and contract AI for everyone else.
//!
//! A character is a root entity with [`Character`], [`Intent`],
//! [`Traversal`], [`TraversalConfig`], a [`CharacterController`] and a
//! `Transform` at the capsule's centre, and a child with [`Body`], [`State`]
//! and a `Transform` at the feet (`-CAPSULE_HEIGHT / 2`), which the skinned
//! model hangs from. Animation, wardrobe, cloth, seats, rides and ragdolls
//! live in their own modules and read the components defined here.
//!
//! This is the port of the Go `character` package's movement half:
//! `character.go`, `locomotion.go`, `traversal.go`, `contacts.go` (the pose
//! geometry) and `health.go`, with illusion's Jolt character controller
//! rebuilt on Avian's shape casts in [`controller`].

pub mod contacts;
pub mod controller;
pub mod health;
pub mod locomotion;
pub mod ragdoll;
pub mod traversal;

use bevy::prelude::*;

pub use contacts::{
    BoneInfo, BoneTransform, ContactPlane, ContactPoint, ContactRig, LADDER_DEPTH, LimbContact,
    fit_limb, fit_pose, fit_torso, make_contact_rig, plane_distance,
};
pub use controller::{CharacterController, CharacterPhysics, ControllerState, RayHit};
pub use health::{CharacterBody, Downed, Health, LifeState, knock_down, revive};
pub use locomotion::{Controls, View, relative};
pub use ragdoll::Ragdoll;
pub use traversal::{RungSpacing, Traversal, TraversalConfig};

/// The bind pose's height the models are scaled to, and the capsule that
/// stands in for a character: 1.8 m tall, 30 cm across.
pub const BODY_HEIGHT: f32 = 1.75;
pub const CAPSULE_HEIGHT: f32 = 1.8;
pub const CAPSULE_RADIUS: f32 = 0.3;

/// Anim is what a character's body is doing: a locomotion loop, a jump, or a
/// one-shot action. The order is the Go `Anim`'s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Anim {
    #[default]
    Idle,
    Walk,
    Run,
    /// Airborne after standing still (or falling off something).
    Jump,
    /// Airborne after moving.
    RunJump,

    // One-shots: they play once, the character pulling up for them. Only a
    // fall cuts one short; wanting to move skips its recovery.
    Interact,
    /// With the left hand.
    Punch,
    /// Its mirror image: punches alternate hands.
    PunchRight,
    PickUp,

    Slide,
    Roll,
    LadderClimb,
    Vault,
    Mantle,
    WallKick,
    /// Its mirror image, for a wall on the right.
    WallKickRight,
    /// Falling after a wall kick, until landing.
    WallFall,
    WallFallRight,
    /// Touching down after a wall kick.
    WallLand,
    Crouch,
    StandUp,
    LadderExit,
    LadderEnter,
    /// Fall loops after a jump's clip reaches touchdown in the air: falling
    /// from higher than it jumped, legs swinging, until it lands.
    Fall,
    /// StairsUp and StairsDown walk (or run) up and down stairs, a foot on
    /// each step: their clips' cycles are set by how high the feet are, not
    /// by time.
    StairsUp,
    StairsDown,
    /// Drive sits at a wheel, and Ride astride a bike or a trike.
    Drive,
    Ride,
    /// CrouchWalk walks crouched, under a low ceiling: its clip's cycle is
    /// set by how far the body has gone, not by time.
    CrouchWalk,
    /// SitDown, Sitting and SitUp sit on a bench, a stool or a bunk and get
    /// up again.
    SitDown,
    Sitting,
    SitUp,
    /// Held poses, played standing still for as long as `Intent::hold` asks:
    /// kneeling at a machine to work on it, talking, dancing.
    Fix,
    Talk,
    Dance,
}

impl Anim {
    /// Whether this is a pose held for as long as `Intent::hold` asks.
    pub fn held(self) -> bool {
        matches!(self, Anim::Fix | Anim::Talk | Anim::Dance)
    }

    /// Whether this is a jump, or the fall after one.
    pub fn airborne(self) -> bool {
        matches!(self, Anim::Jump | Anim::RunJump | Anim::Fall)
    }

    /// Whether this is an action that plays once.
    pub fn one_shot(self) -> bool {
        self >= Anim::Interact && self <= Anim::PickUp
    }
}

impl std::fmt::Display for Anim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Anim::Idle => "idle",
            Anim::Walk => "walk",
            Anim::Run => "run",
            Anim::Jump => "jump",
            Anim::RunJump => "running jump",
            Anim::Interact => "interact",
            Anim::Punch | Anim::PunchRight => "punch",
            Anim::PickUp => "pick up",
            Anim::Slide => "slide",
            Anim::Roll => "roll",
            Anim::LadderClimb => "ladder climb",
            Anim::Vault => "vault",
            Anim::Mantle => "mantle",
            Anim::WallKick | Anim::WallKickRight => "wall kick",
            Anim::WallLand => "wall kick landing",
            Anim::WallFall | Anim::WallFallRight => "wall kick fall",
            Anim::StandUp => "stand up",
            Anim::LadderExit => "ladder exit",
            Anim::LadderEnter => "ladder entry",
            Anim::Crouch => "crouch",
            Anim::Fall => "fall",
            Anim::StairsUp => "stairs up",
            Anim::StairsDown => "stairs down",
            Anim::Drive => "drive",
            Anim::Ride => "ride",
            Anim::CrouchWalk => "crouch walk",
            Anim::SitDown => "sit down",
            Anim::Sitting => "sitting",
            Anim::SitUp => "get up",
            Anim::Fix => "fix",
            Anim::Talk => "talk",
            Anim::Dance => "dance",
        })
    }
}

/// Character is how a character moves. It goes on the root entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// Units per second.
    pub walk_speed: f32,
    pub run_speed: f32,
    /// Upward speed at take-off.
    pub jump_speed: f32,
    /// Ease is how quickly its speed follows a change, walking to running
    /// and back, starting and stopping: each second it closes the gap by
    /// the share 1 - e^-Ease, so it eases in as it gets close. Accel caps
    /// how hard that pushes, in units per second per second, so a start isn't
    /// a jolt. It slows down twice as quickly as it speeds up.
    pub ease: f32,
    pub accel: f32,
    /// TurnSpeed is the fastest the body turns to face where it's going, in
    /// radians per second, standing (the faster it goes, the slower it
    /// turns), and TurnAccel how fast it gets up to that speed and back
    /// down.
    pub turn_speed: f32,
    pub turn_accel: f32,
}

impl Default for Character {
    /// An ordinary person. The speeds are the Mixamo walk and run's own,
    /// measured from how fast a planted foot slides under the body. A walk
    /// turns about in two thirds of a second.
    fn default() -> Self {
        Character {
            walk_speed: 1.6,
            run_speed: 4.6,
            jump_speed: 4.0,
            ease: 5.0,
            accel: 8.0,
            turn_speed: 7.0,
            turn_accel: 30.0,
        }
    }
}

/// Intent is what a character wants to do. `player_input` writes the
/// player's; the character systems carry it out.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    /// The direction to go on the XZ plane, at most 1 long; zero stands
    /// still. In the air it can only veer a character a little. (Go's
    /// `Move`; a keyword here.)
    pub move_dir: Vec3,
    pub run: bool,
    /// Asks for a jump; it's cleared at the next physics step, and ignored
    /// while an action plays.
    pub jump: bool,
    /// Edge-triggered requests consumed by traversal.
    pub slide: bool,
    pub roll: bool,
    /// Holds a crouch for as long as it's set (and longer, under something
    /// low, until there's room to stand).
    pub crouch: bool,
    /// Asks for a one-shot action; Idle means none. It's cleared as soon as
    /// it's read, so an ignored request isn't kept for later. Airborne
    /// characters, characters already acting, and skins without the clip
    /// ignore it. A moving character pulls up for an action, and plays it to
    /// its end before another can start; Move waits until then, or until the
    /// action is far enough along to skip its recovery and move on.
    pub act: Anim,
    /// Asks for a held pose (Fix, Talk, Dance) while it stands still; Idle
    /// means none. It's kept until it's changed: moving, jumping, sliding,
    /// rolling or acting drops it.
    pub hold: Anim,
}

/// Body marks the child entity that draws a character.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Body;

/// State is the body's current animation. It goes on the Body. The fields
/// the Go code keeps private are public here, for the animation module.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub current: Anim,
    pub downed: bool,
    /// Index into the roster's skins.
    pub skin: usize,
    /// Seconds off the ground.
    pub air: f32,
    /// How fast the body is turning, in radians per second.
    pub turn: f32,
    /// Seconds left counting as moving on after an action.
    pub resume: f32,
    /// Which way it's going on stairs (1 up, -1 down, 0 not), and how much
    /// longer it counts as on them since it last was, so a moment off the
    /// slope at the top or bottom step doesn't flicker to a walk and back.
    pub stairs: i8,
    pub stairs_left: f32,
    /// Whether the next punch is with the right hand.
    pub right_punch: bool,
    /// How far through its cycle a crouched walk is, 0 to 1, and how long
    /// into its clip a still crouch is.
    pub crouch_phase: f32,
    pub crouch_idle: f32,
    /// Whether it's hidden, seated out of sight.
    pub hidden: bool,

    /// Off the ground (as the physics step last saw it), and the way it was
    /// going and facing when it left: in the air it can only veer a little
    /// from those.
    pub aloft: bool,
    pub launch: Vec3,
    pub launch_yaw: f32,
}

/// Player marks the character the keyboard controls.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Player;

/// Sets in the Update schedule, in the order they run, and the fixed-step
/// set the movement runs in.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CharacterSystems {
    /// Writes the player's Intent from the keyboard. Systems that take a
    /// key press for themselves (getting into a vehicle with E, say) run
    /// after it and clear what it asked for.
    Input,
    /// Carries out what characters want: turns and animates them.
    Act,
    /// FixedUpdate: traversal, then locomotion, then the controller step
    /// that moves the capsule. Physics-coupled logic that must see the
    /// step's result (impacts, ragdolls) runs after it.
    Move,
}

/// Runs the character movement systems. It needs `EarthPhysicsPlugin`.
pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>()
            .init_resource::<View>()
            .configure_sets(Update, CharacterSystems::Act.after(CharacterSystems::Input))
            .add_systems(
                FixedUpdate,
                ragdoll::resist_rolling.after(CharacterSystems::Move),
            )
            .add_systems(
                FixedPostUpdate,
                ragdoll::step_ragdolls.after(avian3d::prelude::PhysicsSystems::Writeback),
            )
            .add_systems(
                FixedUpdate,
                (
                    health::fall_incapacitated,
                    controller::prepare_characters,
                    traversal::traverse,
                    locomotion::locomote,
                    controller::step_characters,
                )
                    .chain()
                    .in_set(CharacterSystems::Move),
            )
            .add_systems(
                Update,
                (
                    locomotion::player_input.in_set(CharacterSystems::Input),
                    health::stop_downed
                        .after(CharacterSystems::Input)
                        .before(CharacterSystems::Act),
                    locomotion::face.in_set(CharacterSystems::Act),
                ),
            );
    }
}

/// The feet of a character whose capsule centre is at `center`.
pub fn feet_of(center: Vec3, height: f32) -> Vec3 {
    center - Vec3::Y * (height / 2.0)
}

pub(crate) fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// Brings `a` into (-π, π].
pub fn wrap_angle(a: f32) -> f32 {
    let two_pi = 2.0 * std::f32::consts::PI;
    // Go's math.Remainder: IEEE remainder, the result nearest zero.
    let mut a = (a as f64 - (a as f64 / two_pi as f64).round() * two_pi as f64) as f32;
    if a <= -std::f32::consts::PI {
        a += two_pi;
    }
    a
}

/// The turn about Up of a rotation that has only that.
pub fn yaw_of(q: Quat) -> f32 {
    let f = q * Vec3::Z;
    f.x.atan2(f.z)
}

pub(crate) fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

pub(crate) fn direction(v: Vec3) -> Vec3 {
    if v.length_squared() < 0.0001 {
        Vec3::ZERO
    } else {
        v.normalize()
    }
}
