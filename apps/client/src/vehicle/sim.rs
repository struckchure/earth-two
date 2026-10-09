//! A body on wheels, as illusion's `physics.Vehicle` (Jolt's
//! VehicleConstraint) had it, stepped on an Avian body: each wheel casts
//! against the world, rides on a sprung suspension and grips with a tyre,
//! and an engine drives them through a gearbox and differentials. Set
//! [`VehicleInput`] to drive it, and read [`VehicleState`] for its wheels.
//!
//! Jolt solves the wheels as constraints inside its solver; Avian has no
//! hook for that, so the same formulas run just before the physics step,
//! in `FixedUpdate`: the suspension as a spring force over the step, the
//! tyres as impulses limited by the friction the springs can carry, the
//! engine, gearbox and brakes on the wheels' own angular velocities, and
//! the pitch/roll limit and a motorcycle's lean as angular impulses. The
//! constants are Jolt's defaults where the game left them.
//!
//! It's ignored while the body is Static or Kinematic, so a parked vehicle
//! can be made Static and woken by switching back to Dynamic. Vectors are in
//! the body's local space, which faces Forward.

use avian3d::{
    dynamics::rigid_body::forces::{ForcesItem, ReadRigidBodyForces, WriteRigidBodyForces},
    math::SymmetricTensor,
    prelude::*,
};
use bevy::prelude::*;

use crate::physics::FIXED_HZ;

/// One of a [`Vehicle`]'s wheels. Zero values mean Jolt's defaults where
/// zero wouldn't work.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wheel {
    /// Where the suspension is attached: the wheel's center with the
    /// suspension fully raised is `suspension_min` below it.
    pub position: Vec3,
    pub radius: f32, // 0 means 0.3
    pub width: f32,  // 0 means 0.1
    /// How far the wheel's center hangs below `position` fully raised and
    /// fully drooped. A zero `suspension_max` means `suspension_min` + 0.2.
    pub suspension_min: f32,
    pub suspension_max: f32,
    /// Frequency (Hz) and damping (0..1) make the suspension's spring; 0
    /// means 1.5 Hz and 0.5. The stiffness then comes from the mass at each
    /// wheel's contact.
    pub frequency: f32,
    pub damping: f32,
    /// Stiffness (N/m) and damping rate (N·s/m), when `stiffness` is set,
    /// make the spring instead, so it sinks by exactly its load over
    /// stiffness.
    pub stiffness: f32,
    pub damping_rate: f32,
    /// How far the wheel steers (radians, up to π/2); negative steers it
    /// the other way, as a rear axle does to turn tighter.
    pub max_steer: f32,
    /// The most torque the brakes put on it (Nm); 0 means 1500.
    pub brake: f32,
    /// The most torque the hand brake puts on it (Nm); 0 means none.
    pub hand_brake: f32,
    /// The wheel's moment of inertia (kg m²); 0 means 0.9.
    pub inertia: f32,
    /// Scales the tyre's friction, forward and sideways; 0 means 1.
    pub grip: f32,
    /// Points down the suspension, and up the axis the wheel steers around;
    /// zero means straight down and straight up. A bike's fork tilts both.
    pub suspension_dir: Vec3,
    pub steering_axis: Vec3,
    /// The axes of the wheel's model that [`VehicleState`] turns to face
    /// the wheel's right and up; zero means the vehicle's. One model can
    /// serve both sides of a vehicle by flipping `model_right` for the
    /// wheels on one side.
    pub model_right: Vec3,
    pub model_up: Vec3,
}

/// A [`Vehicle`]'s engine. Zero values mean Jolt's defaults: 500 Nm from
/// 1000 to 6000 rpm.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Engine {
    pub max_torque: f32,
    pub min_rpm: f32,
    pub max_rpm: f32,
}

/// A [`Vehicle`]'s automatic gearbox. Zero values mean Jolt's defaults:
/// five gears, shifting up at 4000 rpm and down at 2000.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transmission {
    pub gears: Vec<f32>, // forward gear ratios, first gear first
    pub reverse: f32,    // reverse gear ratio
    pub shift_up_rpm: f32,
    pub shift_down_rpm: f32,
    pub clutch_strength: f32,
}

/// Drives a pair of a [`Vehicle`]'s wheels, by index into its wheels; None
/// means no wheel. A vehicle with none doesn't drive.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Differential {
    pub left: Option<usize>,
    pub right: Option<usize>,
    /// How many times the wheels turn slower than the gearbox; 0 means
    /// 3.42.
    pub ratio: f32,
    /// The share of torque that goes to the right wheel; 0 means an even
    /// 0.5.
    pub split: f32,
    /// This differential's share of the engine's torque; 0 means an even
    /// share among all the differentials.
    pub torque_ratio: f32,
    /// The most one wheel turns faster than the other before torque goes to
    /// the slower one; 0 means 1.4.
    pub limited_slip: f32,
}

/// Ties the suspension of two of a [`Vehicle`]'s wheels together, keeping
/// it flatter in turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AntiRollBar {
    pub left: usize,
    pub right: usize,
    pub stiffness: f32, // N/m; 0 means 1000
}

/// A motorcycle's balance. Zero values mean Jolt's defaults.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lean {
    pub max_angle: f32, // radians; 0 means 45°
    pub spring: f32,
    pub damping: f32,
}

/// How a [`Vehicle`]'s wheels find the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelTester {
    /// Sweeps each wheel's cylinder: the most accurate.
    #[default]
    CastCylinder,
    /// Casts a ray down each suspension: the cheapest, but wheels drop into
    /// gaps narrower than they are.
    CastRay,
    /// Sweeps a sphere as wide as the narrowest wheel.
    CastSphere,
}

/// Puts a Dynamic body on wheels. Replacing the component rebuilds the
/// vehicle; changing it in place after that does nothing.
#[derive(Component, Debug, Clone, PartialEq, Default)]
#[require(VehicleInput, VehicleState)]
pub struct Vehicle {
    pub wheels: Vec<Wheel>,
    pub engine: Engine,
    pub transmission: Transmission,
    pub differentials: Vec<Differential>,
    pub anti_roll_bars: Vec<AntiRollBar>,
    /// The body's up and forward; zero means +Y and +Z.
    pub up: Vec3,
    pub forward: Vec3,
    /// Keeps the vehicle from tipping further than this (radians); 0 means
    /// no limit.
    pub max_pitch_roll: f32,
    /// Makes a two-wheeled vehicle a motorcycle, which leans into turns and
    /// stays up by itself.
    pub lean: Option<Lean>,
    /// How wheels find the ground.
    pub tester: WheelTester,
}

/// What drives a [`Vehicle`], set each frame. `forward` and `right` run
/// from -1 to 1 (pushing forward against the direction of travel brakes,
/// then reverses), `brake` and `hand_brake` from 0 to 1.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct VehicleInput {
    pub forward: f32,
    pub right: f32,
    pub brake: f32,
    pub hand_brake: f32,
}

/// A [`Vehicle`] as last simulated, written after every step while its
/// body is Dynamic.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct VehicleState {
    pub wheels: Vec<WheelState>,
    pub rpm: f32,
    pub gear: i32,       // -1 reverse, 0 neutral, 1 first...
    pub speed: f32,      // along the vehicle's forward, m/s
    pub touching: usize, // wheels on something
}

/// A wheel as last simulated.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WheelState {
    /// Places the wheel's model relative to the body: its suspension,
    /// steering and spin.
    pub transform: Transform,
    pub spin: f32,       // angular velocity, rad/s, positive rolling forward
    pub steer: f32,      // radians
    pub suspension: f32, // length, m
    pub contact: bool,
}

// Jolt's defaults, for the zero values.
const DEFAULT_WHEEL_RADIUS: f32 = 0.3;
const DEFAULT_WHEEL_WIDTH: f32 = 0.1;
const DEFAULT_FREQUENCY: f32 = 1.5;
const DEFAULT_DAMPING: f32 = 0.5;
const DEFAULT_BRAKE: f32 = 1500.0;
const DEFAULT_WHEEL_INERTIA: f32 = 0.9;
const WHEEL_ANGULAR_DAMPING: f32 = 0.2;
const DEFAULT_MAX_TORQUE: f32 = 500.0;
const DEFAULT_MIN_RPM: f32 = 1000.0;
const DEFAULT_MAX_RPM: f32 = 6000.0;
const ENGINE_INERTIA: f32 = 0.5;
const ENGINE_DAMPING: f32 = 0.2;
const DEFAULT_GEARS: [f32; 5] = [2.66, 1.78, 1.3, 1.0, 0.74];
const DEFAULT_REVERSE: f32 = -2.9;
const DEFAULT_SHIFT_UP: f32 = 4000.0;
const DEFAULT_SHIFT_DOWN: f32 = 2000.0;
const DEFAULT_CLUTCH: f32 = 10.0;
const SWITCH_TIME: f32 = 0.5;
const CLUTCH_RELEASE_TIME: f32 = 0.3;
const SWITCH_LATENCY: f32 = 0.5;
const DEFAULT_DIFF_RATIO: f32 = 3.42;
const DEFAULT_LIMITED_SLIP: f32 = 1.4;
const DEFAULT_ANTI_ROLL: f32 = 1000.0;
const DEFAULT_MAX_LEAN: f32 = std::f32::consts::FRAC_PI_4;
const DEFAULT_LEAN_SPRING: f32 = 5000.0;
const DEFAULT_LEAN_DAMPING: f32 = 1000.0;
const LEAN_SMOOTHING: f32 = 0.8;
/// Avian's friction where a collider has none.
const DEFAULT_GROUND_FRICTION: f32 = 0.5;
/// Passes over the tyres' impulses: Jolt iterates its solver; the tyres of
/// one vehicle only couple through its body, so a few are enough.
const TYRE_ITERATIONS: usize = 6;
/// How much of the tilt past the limit a step takes back.
const PITCH_ROLL_BAUMGARTE: f32 = 0.2;

const RPM_PER_RAD_S: f32 = 60.0 / (2.0 * std::f32::consts::PI);

fn or_default(v: f32, def: f32) -> f32 {
    if v == 0.0 { def } else { v }
}

fn dir_or(v: Vec3, def: Vec3) -> Vec3 {
    if v == Vec3::ZERO { def } else { v.normalize() }
}

/// Jolt's LinearCurve: straight lines between points, clamped at the ends.
fn curve(points: &[(f32, f32)], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    points[points.len() - 1].1
}

const ENGINE_TORQUE_CURVE: [(f32, f32); 3] = [(0.0, 0.8), (0.66, 1.0), (1.0, 0.8)];
const LONGITUDINAL_FRICTION: [(f32, f32); 3] = [(0.0, 0.0), (0.06, 1.2), (0.2, 1.0)];
const LATERAL_FRICTION: [(f32, f32); 3] = [(0.0, 0.0), (3.0, 1.2), (20.0, 1.0)];

/// A wheel's settings with Jolt's defaults filled in.
#[derive(Debug, Clone, Copy)]
struct WheelSettings {
    position: Vec3,
    radius: f32,
    width: f32,
    suspension_min: f32,
    suspension_max: f32,
    frequency: f32,
    damping: f32,
    stiffness: f32,
    damping_rate: f32,
    max_steer: f32,
    brake: f32,
    hand_brake: f32,
    inertia: f32,
    grip: f32,
    suspension_dir: Vec3,
    steering_axis: Vec3,
    wheel_up: Vec3,
    wheel_forward: Vec3,
    model_right: Vec3,
    model_up: Vec3,
}

/// Where a wheel touches the ground this step.
#[derive(Debug, Clone, Copy)]
struct Contact {
    position: Vec3,
    normal: Vec3,
    longitudinal: Vec3,
    lateral: Vec3,
    point_velocity: Vec3,
    friction: f32,
}

/// A wheel's running state.
#[derive(Debug, Clone, Copy, Default)]
struct WheelRun {
    angular_velocity: f32,
    angle: f32,
    steer: f32,
    suspension_length: f32,
    contact: Option<Contact>,
    suspension_lambda: f32,
    longitudinal_lambda: f32,
    lateral_lambda: f32,
    brake_impulse: f32,
    anti_roll_impulse: f32,
    longitudinal_slip: f32,
    lateral_slip: f32,
    longitudinal_friction: f32,
    lateral_friction: f32,
}

#[derive(Debug, Clone, Copy)]
struct LeanRun {
    max_angle: f32,
    spring: f32,
    damping: f32,
    target: Vec3,
    applied_impulse: f32,
}

/// The vehicle as built from its [`Vehicle`], and how it's going.
#[derive(Component, Debug, Clone)]
pub struct VehicleRuntime {
    built_from: Vehicle,
    wheels: Vec<WheelSettings>,
    run: Vec<WheelRun>,
    max_torque: f32,
    min_rpm: f32,
    max_rpm: f32,
    gears: Vec<f32>,
    reverse: f32,
    shift_up: f32,
    shift_down: f32,
    clutch_strength: f32,
    differentials: Vec<Differential>,
    anti_roll_bars: Vec<AntiRollBar>,
    up: Vec3,
    forward: Vec3,
    cos_max_pitch_roll: f32,
    lean: Option<LeanRun>,
    tester: WheelTester,
    rpm: f32,
    gear: i32,
    clutch_friction: f32,
    switch_left: f32,
    release_left: f32,
    latency_left: f32,
    previous_dt: f32,
}

impl VehicleRuntime {
    fn new(v: &Vehicle) -> Self {
        let up = dir_or(v.up, Vec3::Y);
        let forward = dir_or(v.forward, Vec3::Z);
        let right = forward.cross(up);
        let max_rpm = or_default(v.engine.max_rpm, DEFAULT_MAX_RPM);
        let min_rpm = or_default(v.engine.min_rpm, DEFAULT_MIN_RPM);
        let max_rpm = max_rpm.max(min_rpm + 1.0);
        let gears = if v.transmission.gears.is_empty() {
            DEFAULT_GEARS.to_vec()
        } else {
            v.transmission.gears.iter().copied().take(8).collect()
        };
        // Jolt's gearbox needs MinRPM <= down < up < MaxRPM.
        let shift_up = or_default(v.transmission.shift_up_rpm, DEFAULT_SHIFT_UP).min(max_rpm - 1.0);
        let shift_down =
            or_default(v.transmission.shift_down_rpm, DEFAULT_SHIFT_DOWN).min(shift_up - 1.0);
        let wheels: Vec<WheelSettings> = v
            .wheels
            .iter()
            .map(|w| {
                let suspension_min = w.suspension_min.max(0.0);
                let mut suspension_max = w.suspension_max;
                if suspension_max <= suspension_min {
                    suspension_max = suspension_min + 0.2;
                }
                WheelSettings {
                    position: w.position,
                    radius: or_default(w.radius, DEFAULT_WHEEL_RADIUS),
                    width: or_default(w.width, DEFAULT_WHEEL_WIDTH),
                    suspension_min,
                    suspension_max,
                    frequency: or_default(w.frequency, DEFAULT_FREQUENCY),
                    damping: or_default(w.damping, DEFAULT_DAMPING),
                    stiffness: w.stiffness.max(0.0),
                    damping_rate: w.damping_rate.max(0.0),
                    max_steer: w
                        .max_steer
                        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2),
                    brake: or_default(w.brake, DEFAULT_BRAKE),
                    hand_brake: w.hand_brake.max(0.0),
                    inertia: or_default(w.inertia, DEFAULT_WHEEL_INERTIA),
                    grip: or_default(w.grip, 1.0),
                    suspension_dir: dir_or(w.suspension_dir, -up),
                    steering_axis: dir_or(w.steering_axis, up),
                    wheel_up: up,
                    wheel_forward: forward,
                    model_right: dir_or(w.model_right, right),
                    model_up: dir_or(w.model_up, up),
                }
            })
            .collect();
        let n_diffs = v.differentials.len().max(1) as f32;
        let differentials = v
            .differentials
            .iter()
            .map(|d| Differential {
                left: d.left.filter(|&i| i < wheels.len()),
                right: d.right.filter(|&i| i < wheels.len()),
                ratio: or_default(d.ratio, DEFAULT_DIFF_RATIO),
                split: or_default(d.split, 0.5),
                torque_ratio: or_default(d.torque_ratio, 1.0 / n_diffs),
                limited_slip: or_default(d.limited_slip, DEFAULT_LIMITED_SLIP),
            })
            .collect();
        let anti_roll_bars = v
            .anti_roll_bars
            .iter()
            .filter(|b| b.left < wheels.len() && b.right < wheels.len())
            .map(|b| AntiRollBar {
                stiffness: or_default(b.stiffness, DEFAULT_ANTI_ROLL),
                ..*b
            })
            .collect();
        VehicleRuntime {
            built_from: v.clone(),
            run: vec![
                WheelRun {
                    suspension_length: 0.0,
                    ..default()
                };
                wheels.len()
            ],
            wheels,
            max_torque: or_default(v.engine.max_torque, DEFAULT_MAX_TORQUE),
            min_rpm,
            max_rpm,
            gears,
            reverse: -or_default(v.transmission.reverse, DEFAULT_REVERSE).abs(),
            shift_up,
            shift_down,
            clutch_strength: or_default(v.transmission.clutch_strength, DEFAULT_CLUTCH),
            differentials,
            anti_roll_bars,
            up,
            forward,
            cos_max_pitch_roll: if v.max_pitch_roll > 0.0 {
                v.max_pitch_roll.cos()
            } else {
                -1.0
            },
            lean: v.lean.map(|l| LeanRun {
                max_angle: or_default(l.max_angle, DEFAULT_MAX_LEAN),
                spring: or_default(l.spring, DEFAULT_LEAN_SPRING),
                damping: or_default(l.damping, DEFAULT_LEAN_DAMPING),
                target: Vec3::ZERO,
                applied_impulse: 0.0,
            }),
            tester: v.tester,
            rpm: min_rpm,
            gear: 0,
            clutch_friction: 1.0,
            switch_left: 0.0,
            release_left: 0.0,
            latency_left: 0.0,
            previous_dt: 0.0,
        }
    }

    fn current_ratio(&self) -> f32 {
        if self.gear < 0 {
            self.reverse
        } else if self.gear == 0 {
            0.0
        } else {
            self.gears[(self.gear - 1) as usize]
        }
    }

    fn clamp_rpm(&mut self) {
        self.rpm = self.rpm.clamp(self.min_rpm, self.max_rpm);
    }

    fn engine_torque(&self, acceleration: f32) -> f32 {
        let fraction = (self.rpm - self.min_rpm) / (self.max_rpm - self.min_rpm);
        acceleration * self.max_torque * curve(&ENGINE_TORQUE_CURVE, fraction)
    }

    /// Whether the engine is idling and the gearbox not shifting, so the
    /// body may sleep.
    pub fn allow_sleep(&self, input: &VehicleInput) -> bool {
        input.forward == 0.0
            && self.switch_left <= 0.0
            && self.release_left <= 0.0
            && self.latency_left <= 0.0
            && self.rpm <= self.min_rpm
    }

    /// The wheels' local basis, steered.
    fn wheel_basis(&self, i: usize) -> (Vec3, Vec3, Vec3) {
        let s = &self.wheels[i];
        let steer = Quat::from_axis_angle(s.steering_axis, self.run[i].steer);
        let up = steer * s.wheel_up;
        let forward = steer * s.wheel_forward;
        let right = forward.cross(up).normalize();
        let forward = up.cross(right).normalize();
        (forward, up, right)
    }

    /// Where a wheel's model goes, relative to the body: Jolt's
    /// GetWheelLocalTransform.
    fn wheel_local_transform(&self, i: usize) -> Transform {
        let s = &self.wheels[i];
        let w = &self.run[i];
        let wheel_to_rotational =
            Mat3::from_cols(s.model_right, s.model_up, s.model_up.cross(s.model_right)).transpose();
        let (forward, up, right) = self.wheel_basis(i);
        let rotational_to_local = Mat3::from_cols(right, up, forward);
        let rotation = rotational_to_local * Mat3::from_rotation_x(w.angle) * wheel_to_rotational;
        Transform {
            translation: s.position + s.suspension_dir * w.suspension_length,
            rotation: Quat::from_mat3(&rotation).normalize(),
            scale: Vec3::ONE,
        }
    }

    fn wheel_base(&self) -> f32 {
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for s in &self.wheels {
            let value = (s.position + s.suspension_dir * s.suspension_max).dot(self.forward);
            low = low.min(value);
            high = high.max(value);
        }
        high - low
    }

    /// The gear as [`VehicleState`] reports it.
    pub fn gear(&self) -> i32 {
        self.gear
    }
}

/// Builds the runtime for a new or replaced [`Vehicle`].
fn build_vehicles(
    mut commands: Commands,
    vehicles: Query<(Entity, &Vehicle, Option<&VehicleRuntime>), Changed<Vehicle>>,
) {
    for (entity, vehicle, runtime) in &vehicles {
        if runtime.is_some_and(|r| r.built_from == *vehicle) {
            continue;
        }
        commands.entity(entity).insert(VehicleRuntime::new(vehicle));
    }
}

/// A body's velocity at a point, for the ground under a wheel. Another
/// vehicle's chassis counts as still: its velocity is being written.
type GroundBodies<'w, 's> = Query<
    'w,
    's,
    (
        &'static RigidBody,
        &'static Position,
        &'static Rotation,
        &'static ComputedCenterOfMass,
        Option<&'static LinearVelocity>,
        Option<&'static AngularVelocity>,
    ),
    Without<VehicleRuntime>,
>;

#[allow(clippy::type_complexity)]
pub(crate) struct Ground<'w, 's> {
    pub spatial: SpatialQuery<'w, 's>,
    pub collider_of: Query<'w, 's, &'static ColliderOf>,
    pub frictions: Query<'w, 's, &'static Friction>,
    pub sensors: Query<'w, 's, (), With<Sensor>>,
    pub bodies: GroundBodies<'w, 's>,
}

impl Ground<'_, '_> {
    /// The body a hit collider belongs to.
    fn body_of(&self, collider: Entity) -> Option<Entity> {
        if self.bodies.contains(collider) {
            return Some(collider);
        }
        self.collider_of.get(collider).ok().map(|c| c.body)
    }

    fn friction_of(&self, collider: Entity, body: Option<Entity>) -> f32 {
        self.frictions
            .get(collider)
            .ok()
            .or_else(|| body.and_then(|b| self.frictions.get(b).ok()))
            .map(|f| f.dynamic_coefficient)
            .unwrap_or(DEFAULT_GROUND_FRICTION)
    }

    fn point_velocity(&self, body: Option<Entity>, point: Vec3) -> Vec3 {
        let Some((kind, pos, rot, com, lin, ang)) = body.and_then(|b| self.bodies.get(b).ok())
        else {
            return Vec3::ZERO;
        };
        if kind.is_static() {
            return Vec3::ZERO;
        }
        let lin = lin.map(|v| v.0).unwrap_or(Vec3::ZERO);
        let ang = ang.map(|v| v.0).unwrap_or(Vec3::ZERO);
        lin + ang.cross(point - (pos.0 + rot.0 * com.0))
    }
}

/// Effective mass of a body along `axis` at a point `r` from its centre of
/// mass: 1 / (1/m + (r × a) · I⁻¹ (r × a)).
fn effective_mass(inv_mass: f32, inv_inertia: &SymmetricTensor, r: Vec3, axis: Vec3) -> f32 {
    let rxa = r.cross(axis);
    let k = inv_mass + rxa.dot(inv_inertia.mul_vec3(rxa));
    if k > 0.0 { 1.0 / k } else { 0.0 }
}

fn effective_angular_mass(inv_inertia: &SymmetricTensor, axis: Vec3) -> f32 {
    let k = axis.dot(inv_inertia.mul_vec3(axis));
    if k > 0.0 { 1.0 / k } else { 0.0 }
}

/// Solves a·x = b in place by Gaussian elimination with partial pivoting;
/// false if singular.
fn solve_linear(a: &mut [Vec<f32>], b: &mut [f32]) -> bool {
    let n = b.len();
    for col in 0..n {
        let mut pivot = col;
        for row in col + 1..n {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        if a[pivot][col].abs() < 1e-12 {
            return false;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let f = a[row][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            let (before, after) = a.split_at_mut(row);
            for (value, pivot) in after[0][col..n].iter_mut().zip(&before[col][col..n]) {
                *value -= f * pivot;
            }
            b[row] -= f * b[col];
        }
    }
    for col in (0..n).rev() {
        let mut sum = b[col];
        for k in col + 1..n {
            sum -= a[col][k] * b[k];
        }
        b[col] = sum / a[col][col];
    }
    true
}

/// Steps every Dynamic vehicle: Jolt's OnStep, then its velocity solve,
/// as forces and impulses on the Avian body before the physics step.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn step_vehicles(
    mut vehicles: Query<(
        Entity,
        &mut VehicleRuntime,
        &VehicleInput,
        &mut VehicleState,
        &RigidBody,
        &Position,
        &Rotation,
        &ComputedMass,
        &ComputedCenterOfMass,
        &ComputedAngularInertia,
        Option<&RigidBodyColliders>,
        Forces,
    )>,
    spatial: SpatialQuery,
    collider_of: Query<&'static ColliderOf>,
    frictions: Query<&'static Friction>,
    sensors: Query<(), With<Sensor>>,
    bodies: GroundBodies,
    gravity: Res<Gravity>,
    physics_time: Res<Time<Physics>>,
    time: Res<Time>,
) {
    if physics_time.is_paused() {
        return;
    }
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let ground = Ground {
        spatial,
        collider_of,
        frictions,
        sensors,
        bodies,
    };
    for (
        entity,
        mut v,
        input,
        mut state,
        body,
        position,
        rotation,
        mass,
        center_of_mass,
        inertia,
        colliders,
        mut forces,
    ) in &mut vehicles
    {
        if !body.is_dynamic() {
            continue;
        }
        let v = &mut *v;
        let world_up = (-gravity.0).try_normalize().unwrap_or(v.up);
        let rot = rotation.0;
        let pos = position.0;
        let mut excluded = vec![entity];
        if let Some(colliders) = colliders {
            excluded.extend(colliders.iter());
        }

        pre_collide(
            v,
            input,
            forces.linear_velocity(),
            rot,
            world_up,
            gravity.0.length(),
        );
        collide(v, &ground, pos, rot, &excluded);
        anti_roll(v, dt);
        post_collide(v, input, &forces, dt);

        let inv_mass = mass.inverse();
        let inv_inertia = inertia.rotated(rot).inverse();
        let com = pos + rot * center_of_mass.0;
        solve(
            v,
            &mut forces,
            inv_mass,
            &inv_inertia,
            com,
            rot,
            world_up,
            dt,
        );

        // What's simulated, for the models and the HUD.
        state.wheels.clear();
        for i in 0..v.wheels.len() {
            let w = v.run[i];
            state.wheels.push(WheelState {
                transform: v.wheel_local_transform(i),
                spin: w.angular_velocity,
                steer: w.steer,
                suspension: w.suspension_length,
                contact: w.contact.is_some(),
            });
        }
        state.rpm = v.rpm;
        state.gear = v.gear;
        state.speed = forces.linear_velocity().dot(rot * v.forward);
        state.touching = v.run.iter().filter(|w| w.contact.is_some()).count();
        v.previous_dt = dt;
    }
}

/// Steering, and a motorcycle's target lean: the controller's PreCollide.
fn pre_collide(
    v: &mut VehicleRuntime,
    input: &VehicleInput,
    linear_velocity: Vec3,
    rot: Quat,
    world_up: Vec3,
    gravity: f32,
) {
    for i in 0..v.wheels.len() {
        v.run[i].steer = -input.right * v.wheels[i].max_steer;
    }
    let Some(mut lean) = v.lean else {
        return;
    };
    let forward = rot * v.forward;
    let wheel_base = v.wheel_base();

    // Lean toward where the ground and the tyres push: the contact normals
    // weighted by their loads, and the sideways grip.
    let mut target = Vec3::ZERO;
    for w in &v.run {
        if let Some(c) = w.contact {
            target += c.normal * w.suspension_lambda + c.lateral * w.lateral_lambda;
        }
    }
    let target = target.try_normalize().unwrap_or(world_up);
    lean.target = LEAN_SMOOTHING * lean.target + (1.0 - LEAN_SMOOTHING) * target;
    lean.target -= forward * lean.target.dot(forward);
    lean.target = lean.target.try_normalize().unwrap_or(world_up);
    let adjusted_up = (world_up - forward * world_up.dot(forward))
        .try_normalize()
        .unwrap_or(world_up);
    let w_angle = -lean.target.cross(adjusted_up).dot(forward).signum()
        * lean.target.dot(adjusted_up).clamp(-1.0, 1.0).acos();
    if w_angle.abs() > lean.max_angle {
        lean.target =
            Quat::from_axis_angle(forward, w_angle.signum() * lean.max_angle) * adjusted_up;
    }

    // Steering is limited so the bike can't be asked to lean past its most.
    let max_steer_factor = wheel_base * lean.max_angle.tan() * gravity;
    let velocity_sq = linear_velocity.dot(forward).powi(2);
    let strength = input.right.abs();
    let sign = -input.right.signum();
    for i in 0..v.wheels.len() {
        let s = v.wheels[i];
        if s.max_steer == 0.0 {
            continue;
        }
        let cos_caster = s.steering_axis.dot(v.up);
        let mut angle = strength * s.max_steer;
        if velocity_sq > 1e-6 && cos_caster > 1e-6 {
            let max = (max_steer_factor / (velocity_sq * cos_caster))
                .clamp(-1.0, 1.0)
                .asin();
            angle = angle.min(max);
        }
        v.run[i].steer = if input.right == 0.0 {
            0.0
        } else {
            sign * angle
        };
    }
    lean.applied_impulse = 0.0;
    v.lean = Some(lean);
}

/// Finds the ground under each wheel.
fn collide(v: &mut VehicleRuntime, ground: &Ground, pos: Vec3, rot: Quat, excluded: &[Entity]) {
    let filter = SpatialQueryFilter::from_excluded_entities(excluded.iter().copied());
    let not_sensor = |e: Entity| !ground.sensors.contains(e);
    for i in 0..v.wheels.len() {
        let s = v.wheels[i];
        let origin = pos + rot * s.position;
        let direction = rot * s.suspension_dir;
        let Ok(dir) = Dir3::new(direction) else {
            continue;
        };
        let (_, _, right) = v.wheel_basis(i);
        let right = rot * right;

        v.run[i].contact = None;
        v.run[i].suspension_length = s.suspension_max;
        let hit = match v.tester {
            WheelTester::CastRay => {
                let length = s.suspension_max + s.radius;
                ground
                    .spatial
                    .cast_ray_predicate(origin, dir, length, true, &filter, &not_sensor)
                    .map(|h| {
                        (
                            h.entity,
                            origin + direction * h.distance,
                            h.normal,
                            (h.distance - s.radius).max(0.0),
                        )
                    })
            }
            WheelTester::CastSphere => {
                let radius = v
                    .wheels
                    .iter()
                    .map(|w| 0.5 * w.width)
                    .fold(f32::MAX, f32::min);
                let length = s.suspension_max + s.radius - radius;
                let shape = Collider::sphere(radius);
                let config = ShapeCastConfig::from_max_distance(length);
                ground
                    .spatial
                    .cast_shape_predicate(
                        &shape,
                        origin,
                        Quat::IDENTITY,
                        dir,
                        &config,
                        &filter,
                        &not_sensor,
                    )
                    .map(|h| {
                        (
                            h.entity,
                            h.point1,
                            h.normal1,
                            (h.distance + radius - s.radius).max(0.0),
                        )
                    })
            }
            WheelTester::CastCylinder => {
                // The cylinder's axis is its Y; the wheel's is its axle.
                let shape = Collider::cylinder(s.radius, s.width);
                let turn = Quat::from_rotation_arc(Vec3::Y, right);
                let config = ShapeCastConfig::from_max_distance(s.suspension_max);
                ground
                    .spatial
                    .cast_shape_predicate(&shape, origin, turn, dir, &config, &filter, &not_sensor)
                    .map(|h| (h.entity, h.point1, h.normal1, h.distance.max(0.0)))
            }
        };
        let Some((collider, point, normal, length)) = hit else {
            continue;
        };
        let normal = normal.try_normalize().unwrap_or(-direction);
        let body = ground.body_of(collider);
        let length = length.min(s.suspension_max);
        let (forward, _, right_local) = v.wheel_basis(i);
        let forward = rot * forward;
        let right = rot * right_local;
        let mut longitudinal = normal.cross(right);
        if longitudinal.dot(forward) < 0.0 {
            longitudinal = -longitudinal;
        }
        let longitudinal = longitudinal
            .try_normalize()
            .unwrap_or_else(|| normal.any_orthonormal_vector());
        let lateral = longitudinal.cross(normal).normalize();
        v.run[i].suspension_length = length;
        v.run[i].contact = Some(Contact {
            position: point,
            normal,
            longitudinal,
            lateral,
            point_velocity: ground.point_velocity(body, point),
            friction: ground.friction_of(collider, body),
        });
    }
}

fn anti_roll(v: &mut VehicleRuntime, dt: f32) {
    for w in &mut v.run {
        w.anti_roll_impulse = 0.0;
    }
    for bar in v.anti_roll_bars.clone() {
        let (l, r) = (bar.left, bar.right);
        if v.run[l].contact.is_some() && v.run[r].contact.is_some() {
            let difference = v.run[r].suspension_length - v.run[l].suspension_length;
            let impulse = difference * bar.stiffness * dt;
            v.run[l].anti_roll_impulse = -impulse;
            v.run[r].anti_roll_impulse = impulse;
        }
    }
}

/// The engine, gearbox, differentials and brakes on the wheels' spin: the
/// controller's PostCollide.
fn post_collide(v: &mut VehicleRuntime, input: &VehicleInput, forces: &ForcesItem, dt: f32) {
    let old_rpm = v.rpm;

    // Update wheel angles and slips before the engine, as friction will
    // slow the wheels down again.
    for i in 0..v.wheels.len() {
        let s = v.wheels[i];
        let w = &mut v.run[i];
        w.angular_velocity *= (1.0 - WHEEL_ANGULAR_DAMPING * dt).max(0.0);
        w.angle = (w.angle + w.angular_velocity * dt) % (2.0 * std::f32::consts::PI);
        if let Some(c) = w.contact {
            let mut relative = forces.velocity_at_point(c.position) - c.point_velocity;
            relative -= c.normal * c.normal.dot(relative);
            let longitudinal = relative.dot(c.longitudinal);
            let denom = longitudinal.signum() * longitudinal.abs().max(1e-3);
            w.longitudinal_slip = ((w.angular_velocity * s.radius - longitudinal) / denom).abs();
            let long_friction = curve(&LONGITUDINAL_FRICTION, w.longitudinal_slip);
            let len = relative.length();
            w.lateral_slip = if len < 1e-3 {
                0.0
            } else {
                (longitudinal.abs() / len).clamp(-1.0, 1.0).acos()
            };
            let lat_friction = curve(&LATERAL_FRICTION, w.lateral_slip.to_degrees());
            // The tyre's grip, combined with the ground's as Jolt does.
            w.longitudinal_friction = (long_friction * s.grip * c.friction).sqrt();
            w.lateral_friction = (lat_friction * s.grip * c.friction).sqrt();
        } else {
            w.longitudinal_slip = 0.0;
            w.lateral_slip = 0.0;
            w.longitudinal_friction = 0.0;
            w.lateral_friction = 0.0;
        }
    }

    // In auto transmission mode, don't accelerate the engine when switching
    // gears.
    let forward_input = input.forward.abs() * v.clutch_friction;
    v.rpm *= (1.0 - ENGINE_DAMPING * dt).max(0.0);
    v.clamp_rpm();
    let engine_torque = v.engine_torque(forward_input);

    // Driven differentials and their speeds.
    struct DrivenDiff {
        diff: Differential,
        omega: f32,
        torque_ratio: f32,
        temp: f32,
    }
    let mut driven_diffs: Vec<DrivenDiff> = Vec::new();
    let (mut omega_min, mut omega_max) = (f32::MAX, 0.0f32);
    for d in &v.differentials {
        let mut avg = 0.0;
        let mut n = 0;
        for idx in [d.left, d.right].into_iter().flatten() {
            avg += v.run[idx].angular_velocity;
            n += 1;
        }
        if n > 0 {
            let avg = (avg * d.ratio / n as f32).abs();
            driven_diffs.push(DrivenDiff {
                diff: *d,
                omega: avg,
                torque_ratio: d.torque_ratio,
                temp: 0.0,
            });
            omega_min = omega_min.min(avg);
            omega_max = omega_max.max(avg);
        }
    }
    // Limited slip between the differentials, at Jolt's default ratio.
    if omega_max > omega_min && !driven_diffs.is_empty() {
        let mut sum = 0.0;
        for d in &mut driven_diffs {
            d.temp = (omega_max - d.omega) / (omega_max - omega_min);
            sum += d.temp;
        }
        for d in &mut driven_diffs {
            d.temp /= sum;
        }
        let omega_min = omega_min.max(1e-3);
        let omega_max = omega_max.max(1e-3);
        let alpha = ((omega_max / omega_min - 1.0) / (DEFAULT_LIMITED_SLIP - 1.0)).min(1.0);
        for d in &mut driven_diffs {
            d.torque_ratio = (1.0 - alpha) * d.torque_ratio + alpha * d.temp;
        }
    }

    struct DrivenWheel {
        wheel: usize,
        clutch_to_wheel_ratio: f32,
        torque_ratio: f32,
        estimated_impulse: f32,
    }
    let transmission_ratio = v.current_ratio();
    let mut driven: Vec<DrivenWheel> = Vec::new();
    for dd in &driven_diffs {
        let d = dd.diff;
        let ratio = transmission_ratio * d.ratio;
        match (d.left, d.right) {
            (Some(l), Some(r)) => {
                let (ratio_l, ratio_r) =
                    torque_split(&d, v.run[l].angular_velocity, v.run[r].angular_velocity);
                driven.push(DrivenWheel {
                    wheel: l,
                    clutch_to_wheel_ratio: ratio,
                    torque_ratio: dd.torque_ratio * ratio_l,
                    estimated_impulse: 0.0,
                });
                driven.push(DrivenWheel {
                    wheel: r,
                    clutch_to_wheel_ratio: ratio,
                    torque_ratio: dd.torque_ratio * ratio_r,
                    estimated_impulse: 0.0,
                });
            }
            (Some(w), None) | (None, Some(w)) => driven.push(DrivenWheel {
                wheel: w,
                clutch_to_wheel_ratio: ratio,
                torque_ratio: dd.torque_ratio,
                estimated_impulse: 0.0,
            }),
            (None, None) => {}
        }
    }

    let mut solved = false;
    if !driven.is_empty() {
        // The clutch couples the engine to the driven wheels; the system is
        // stiff, so it's integrated implicitly: Jolt's a·w(t+dt) = b.
        let n = driven.len() + 1;
        let engine = n - 1;
        let mut a = vec![vec![0.0f32; n]; n];
        let mut b = vec![0.0f32; n];
        let num = driven.len() as f32;
        let w_engine = v.rpm / RPM_PER_RAD_S;
        let clutch_strength = if transmission_ratio != 0.0 {
            v.clutch_friction * v.clutch_strength
        } else {
            0.0
        };
        let dt_div_ie = dt / ENGINE_INERTIA;
        let impulse_scale = if v.previous_dt > 0.0 {
            dt / v.previous_dt
        } else {
            0.0
        };
        for i in 0..driven.len() {
            let s = v.wheels[driven[i].wheel];
            let w = v.run[driven[i].wheel];
            let s_r = clutch_strength * driven[i].clutch_to_wheel_ratio;
            let dt_s_r_f_div_iw = dt * s_r * driven[i].torque_ratio / s.inertia;
            for j in 0..driven.len() {
                a[i][j] = dt_s_r_f_div_iw * driven[j].clutch_to_wheel_ratio / num;
            }
            a[i][i] += 1.0;
            a[i][engine] = -dt_s_r_f_div_iw;

            let mut dt_tw = 0.0;
            let brake_torque = input.brake * s.brake + input.hand_brake * s.hand_brake;
            if brake_torque > 0.0 {
                let sign = if w.angular_velocity != 0.0 {
                    w.angular_velocity.signum()
                } else {
                    transmission_ratio.signum()
                };
                dt_tw = sign * dt * brake_torque;
            }
            if w.contact.is_some() {
                // The ground's torque isn't known yet: last step's impulse
                // estimates it.
                dt_tw += impulse_scale * w.longitudinal_lambda * s.radius;
            }
            driven[i].estimated_impulse = dt_tw;
            b[i] = w.angular_velocity - dt_tw / s.inertia;
            a[engine][i] = -dt_div_ie * s_r / num;
        }
        a[engine][engine] = 1.0 + dt_div_ie * clutch_strength;
        b[engine] = w_engine + dt_div_ie * engine_torque;

        if solve_linear(&mut a, &mut b) {
            for (i, d) in driven.iter().enumerate() {
                let s = v.wheels[d.wheel];
                // The estimated torque is applied for real by the brakes
                // and the ground below, so it's undone here.
                v.run[d.wheel].angular_velocity = b[i] + d.estimated_impulse / s.inertia;
            }
            v.rpm = b[engine] * RPM_PER_RAD_S;
            v.clamp_rpm();
            solved = true;
        }
    }
    if !solved {
        // Engine not connected to wheels: all torque spins the engine.
        v.rpm += RPM_PER_RAD_S * engine_torque * dt / ENGINE_INERTIA;
        v.clamp_rpm();
    }

    // Only shift up when not slipping and the revs are rising: after a jump
    // the revs are high but falling once the wheels land.
    let slipping = driven.iter().any(|d| {
        let w = v.run[d.wheel];
        d.torque_ratio > 0.0 && (w.contact.is_none() || w.longitudinal_slip > 0.1)
    });
    let can_shift_up = !slipping && v.rpm >= old_rpm;
    update_transmission(v, dt, input.forward, can_shift_up);

    // Braking.
    for i in 0..v.wheels.len() {
        let s = v.wheels[i];
        let w = &mut v.run[i];
        let brake_torque = input.brake * s.brake + input.hand_brake * s.hand_brake;
        if brake_torque > 0.0 {
            let to_lock = w.angular_velocity.abs() * s.inertia / dt;
            if brake_torque > to_lock {
                w.angular_velocity = 0.0;
                w.brake_impulse = (brake_torque - to_lock) * dt / s.radius;
            } else {
                w.angular_velocity -= w.angular_velocity.signum() * brake_torque * dt / s.inertia;
                w.brake_impulse = 0.0;
            }
        } else {
            w.brake_impulse = 0.0;
        }
    }
}

/// How a differential shares torque between its wheels: evenly by its
/// split, then toward the slower wheel as the other spins up.
fn torque_split(d: &Differential, left: f32, right: f32) -> (f32, f32) {
    let mut l = 1.0 - d.split;
    let mut r = d.split;
    let omega_l = left.abs().max(1e-3);
    let omega_r = right.abs().max(1e-3);
    let (omega_min, omega_max) = (omega_l.min(omega_r), omega_l.max(omega_r));
    let alpha = ((omega_max / omega_min - 1.0) / (d.limited_slip - 1.0)).min(1.0);
    let one_min = 1.0 - alpha;
    if omega_l < omega_r {
        l = l * one_min + alpha;
        r *= one_min;
    } else {
        l *= one_min;
        r = r * one_min + alpha;
    }
    (l, r)
}

/// Jolt's automatic gearbox.
fn update_transmission(v: &mut VehicleRuntime, dt: f32, forward: f32, can_shift_up: bool) {
    let old = v.gear;
    if v.gear == 0 || forward * (v.gear as f32) < 0.0 {
        // In neutral, or changing between forward and reverse.
        v.gear = if forward > 0.0 {
            1
        } else if forward < 0.0 {
            -1
        } else {
            0
        };
    } else if v.latency_left == 0.0 {
        if can_shift_up && v.rpm > v.shift_up {
            if v.gear < 0 {
                // One reverse gear.
            } else if (v.gear as usize) < v.gears.len() {
                v.gear += 1;
            }
        } else if v.rpm < v.shift_down {
            if v.gear < 0 {
                let max_gear = if forward != 0.0 { -1 } else { 0 };
                if v.gear < max_gear {
                    v.gear += 1;
                }
            } else {
                let min_gear = if forward != 0.0 { 1 } else { 0 };
                if v.gear > min_gear {
                    v.gear -= 1;
                }
            }
        }
    }
    if old != v.gear {
        v.switch_left = if old != 0 { SWITCH_TIME } else { 0.0 };
        v.release_left = CLUTCH_RELEASE_TIME;
        v.latency_left = SWITCH_LATENCY;
        v.clutch_friction = 0.0;
    } else if v.switch_left > 0.0 {
        v.switch_left = (v.switch_left - dt).max(0.0);
        v.clutch_friction = 0.0;
    } else if v.release_left > 0.0 {
        v.release_left = (v.release_left - dt).max(0.0);
        v.clutch_friction = 1.0 - v.release_left / CLUTCH_RELEASE_TIME;
    } else {
        v.clutch_friction = 1.0;
        v.latency_left = (v.latency_left - dt).max(0.0);
    }
}

/// The suspension, the tyres, the pitch/roll limit and a motorcycle's lean
/// on the body.
#[allow(clippy::too_many_arguments)]
fn solve(
    v: &mut VehicleRuntime,
    forces: &mut ForcesItem,
    inv_mass: f32,
    inv_inertia: &SymmetricTensor,
    com: Vec3,
    rot: Quat,
    world_up: Vec3,
    dt: f32,
) {
    // Suspension: a spring and damper along the contact normal, pushing
    // only. Over the step as a force; its impulse is what the tyre can grip
    // with.
    for i in 0..v.wheels.len() {
        let s = v.wheels[i];
        let w = &mut v.run[i];
        w.suspension_lambda = 0.0;
        let Some(c) = w.contact else {
            continue;
        };
        if s.suspension_max <= s.suspension_min {
            continue;
        }
        let (mut stiffness, mut damping) = if s.stiffness > 0.0 {
            (s.stiffness, s.damping_rate)
        } else {
            // From the mass the spring carries at its wheel.
            let force_point =
                s.position + 0.5 * (s.suspension_min + s.suspension_max) * s.suspension_dir;
            let local_inv_inertia = inv_inertia_local(inv_inertia, rot);
            let m_eff = effective_mass(inv_mass, &local_inv_inertia, force_point, -v.up);
            let omega = 2.0 * std::f32::consts::PI * s.frequency;
            (m_eff * omega * omega, 2.0 * m_eff * s.damping * omega)
        };
        let ws_direction = rot * s.suspension_dir;
        let cos_angle = ws_direction.dot(-c.normal).max(0.1);
        stiffness /= cos_angle;
        damping /= cos_angle;
        let compression = w.suspension_length - s.suspension_max;
        let approach = (forces.velocity_at_point(c.position) - c.point_velocity).dot(c.normal);
        let mut force = -stiffness * compression - damping * approach;
        force += w.anti_roll_impulse / dt;
        let force = force.max(0.0);
        forces.apply_force_at_point(force * c.normal, c.position);
        w.suspension_lambda = force * dt;

        // Bottomed out: the wheel can't come up past its minimum, so the
        // body takes the hit.
        if w.suspension_length < s.suspension_min && approach < 0.0 {
            let m_eff = effective_mass(inv_mass, inv_inertia, c.position - com, c.normal);
            let lambda = -approach * m_eff;
            forces.apply_linear_impulse_at_point(lambda * c.normal, c.position);
            w.suspension_lambda += lambda;
        }
    }

    // Tyres. Driving wheels push by the engine's torque on them; braking
    // and sideways grip hold the contact still, as far as friction allows.
    let n = v.wheels.len();
    let mut max_long = vec![0.0f32; n];
    let mut max_lat = vec![0.0f32; n];
    let mut brake_range = vec![(0.0f32, 0.0f32); n];
    for i in 0..n {
        let s = v.wheels[i];
        let w = &mut v.run[i];
        w.longitudinal_lambda = 0.0;
        w.lateral_lambda = 0.0;
        let Some(c) = w.contact else {
            continue;
        };
        max_long[i] = w.longitudinal_friction * w.suspension_lambda;
        max_lat[i] = w.lateral_friction * w.suspension_lambda;
        let relative = forces.velocity_at_point(c.position) - c.point_velocity;
        let longitudinal = relative.dot(c.longitudinal);
        if w.brake_impulse != 0.0 {
            // Brakes never push the vehicle the other way.
            let brake = w.brake_impulse.min(max_long[i]);
            brake_range[i] = if longitudinal >= 0.0 {
                (-brake, 0.0)
            } else {
                (0.0, brake)
            };
        } else {
            // Spin the ground and wheel to the same speed in one step, as
            // far as the tyre grips.
            let desired = longitudinal / s.radius;
            let linear = (w.angular_velocity - desired) * s.inertia / s.radius;
            let lambda = linear.clamp(-max_long[i], max_long[i]);
            forces.apply_linear_impulse_at_point(lambda * c.longitudinal, c.position);
            w.longitudinal_lambda = lambda;
            w.angular_velocity -= lambda * s.radius / s.inertia;
        }
    }
    for _ in 0..TYRE_ITERATIONS {
        for i in 0..n {
            let w = &mut v.run[i];
            let Some(c) = w.contact else {
                continue;
            };
            if w.brake_impulse != 0.0 {
                let m_eff = effective_mass(inv_mass, inv_inertia, c.position - com, c.longitudinal);
                let relative = forces.velocity_at_point(c.position) - c.point_velocity;
                let lambda = -m_eff * relative.dot(c.longitudinal);
                let (lo, hi) = brake_range[i];
                let total = (w.longitudinal_lambda + lambda).clamp(lo, hi);
                let delta = total - w.longitudinal_lambda;
                forces.apply_linear_impulse_at_point(delta * c.longitudinal, c.position);
                w.longitudinal_lambda = total;
            }
            let m_eff = effective_mass(inv_mass, inv_inertia, c.position - com, c.lateral);
            let relative = forces.velocity_at_point(c.position) - c.point_velocity;
            let lambda = -m_eff * relative.dot(c.lateral);
            let total = (w.lateral_lambda + lambda).clamp(-max_lat[i], max_lat[i]);
            let delta = total - w.lateral_lambda;
            forces.apply_linear_impulse_at_point(delta * c.lateral, c.position);
            w.lateral_lambda = total;
        }
    }

    // Pitch and roll: held at the limit, and brought back under it.
    if v.cos_max_pitch_roll > -1.0 {
        let vehicle_up = rot * v.up;
        let cos = world_up.dot(vehicle_up);
        if cos < v.cos_max_pitch_roll
            && let Some(axis) = world_up.cross(vehicle_up).try_normalize()
        {
            let over = cos.clamp(-1.0, 1.0).acos() - v.cos_max_pitch_roll.acos();
            let tilting = forces.angular_velocity().dot(axis);
            let m_eff = effective_angular_mass(inv_inertia, axis);
            let lambda = (m_eff * (tilting + PITCH_ROLL_BAUMGARTE * over / dt)).max(0.0);
            forces.apply_angular_impulse(-lambda * axis);
        }
    }

    // A motorcycle leans to its target with a spring, and the wheels are
    // kept from pushing the ground by the same impulse.
    if let Some(mut lean) = v.lean {
        let all_in_contact = v
            .run
            .iter()
            .all(|w| w.contact.is_some() && w.suspension_lambda > 0.0);
        if all_in_contact {
            let forward = rot * v.forward;
            let up = rot * v.up;
            let d_angle = -lean.target.cross(up).dot(forward).signum()
                * lean.target.dot(up).clamp(-1.0, 1.0).acos();
            let ddt_angle = forces.angular_velocity().dot(forward);
            let total = (lean.spring * d_angle - lean.damping * ddt_angle) * dt;
            let old_w = forces.angular_velocity();
            let delta = total - lean.applied_impulse;
            forces.apply_angular_impulse(delta * forward);
            lean.applied_impulse = total;
            let dw = forces.angular_velocity() - old_w;
            let mut linear_acceleration = Vec3::ZERO;
            let mut total_lambda = 0.0;
            for w in &v.run {
                if let Some(c) = w.contact {
                    total_lambda += w.suspension_lambda;
                    linear_acceleration += w.suspension_lambda * dw.cross(c.position - com);
                }
            }
            if total_lambda > 0.0 {
                forces.apply_linear_impulse(-linear_acceleration / (total_lambda * inv_mass));
            }
        }
        v.lean = Some(lean);
    }
}

/// The world inverse inertia in the body's frame.
fn inv_inertia_local(inv_inertia: &SymmetricTensor, rot: Quat) -> SymmetricTensor {
    let r = Mat3::from_quat(rot);
    let m = r.transpose() * inv_inertia.to_mat3() * r;
    SymmetricTensor::from_mat3_unchecked(m)
}

/// Steps the wheeled vehicles in `FixedUpdate`, before Avian's step in
/// `FixedPostUpdate`, at the game's 60 Hz.
pub struct VehicleSimPlugin;

/// The vehicle step, for ordering game systems after it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VehicleStep;

impl Plugin for VehicleSimPlugin {
    fn build(&self, app: &mut App) {
        const { assert!(FIXED_HZ > 0.0) };
        app.add_systems(
            FixedUpdate,
            (build_vehicles, step_vehicles).chain().in_set(VehicleStep),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_interpolate_and_clamp() {
        assert_eq!(curve(&ENGINE_TORQUE_CURVE, -1.0), 0.8);
        assert_eq!(curve(&ENGINE_TORQUE_CURVE, 0.66), 1.0);
        assert!((curve(&ENGINE_TORQUE_CURVE, 0.33) - 0.9).abs() < 1e-6);
        assert_eq!(curve(&ENGINE_TORQUE_CURVE, 2.0), 0.8);
        assert_eq!(curve(&LATERAL_FRICTION, 20.0), 1.0);
    }

    #[test]
    fn gaussian_elimination_solves_a_small_system() {
        let mut a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let mut b = vec![5.0, 10.0];
        assert!(solve_linear(&mut a, &mut b));
        assert!(
            (b[0] - 1.0).abs() < 1e-5 && (b[1] - 3.0).abs() < 1e-5,
            "{b:?}"
        );
    }

    #[test]
    fn gearbox_shifts_up_and_down_and_reverses() {
        let mut v = VehicleRuntime::new(&Vehicle {
            transmission: Transmission {
                gears: vec![2.8, 1.9, 1.4],
                reverse: 3.0,
                shift_up_rpm: 5200.0,
                shift_down_rpm: 2400.0,
                ..default()
            },
            ..default()
        });
        assert_eq!(v.gear, 0);
        update_transmission(&mut v, 1.0 / 60.0, 1.0, true);
        assert_eq!(v.gear, 1);
        assert_eq!(v.current_ratio(), 2.8);
        // Straight from neutral the switch is instant; the clutch releases.
        for _ in 0..60 {
            update_transmission(&mut v, 1.0 / 60.0, 1.0, true);
        }
        assert_eq!(v.clutch_friction, 1.0);
        v.rpm = 5500.0;
        update_transmission(&mut v, 1.0 / 60.0, 1.0, true);
        assert_eq!(v.gear, 2);
        // Hold within the shift band while the clutch and latency settle.
        v.rpm = 3500.0;
        for _ in 0..120 {
            update_transmission(&mut v, 1.0 / 60.0, 1.0, true);
        }
        v.rpm = 1000.0;
        update_transmission(&mut v, 1.0 / 60.0, 1.0, true);
        assert_eq!(v.gear, 1);
        update_transmission(&mut v, 1.0 / 60.0, -1.0, true);
        assert_eq!(v.gear, -1);
        assert_eq!(v.current_ratio(), -3.0);
    }

    #[test]
    fn differential_sends_torque_to_the_slower_wheel() {
        let d = Differential {
            split: 0.5,
            limited_slip: 1.4,
            ..default()
        };
        assert_eq!(torque_split(&d, 10.0, 10.0), (0.5, 0.5));
        let (l, r) = torque_split(&d, 10.0, 20.0);
        assert!(l > 0.9 && (l + r - 1.0).abs() < 1e-6, "{l} {r}");
    }
}
