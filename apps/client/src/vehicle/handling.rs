//! How each kind of vehicle drives: `vehicle/handling.go`. Engines are
//! geared for about half a g at the wheels in first: Torque × Gears[0] ×
//! Final over a wheel's radius, against Mass. Its suspension is set up so the
//! wheels hang where the model has them when the vehicle stands at rest.

use bevy::prelude::*;

use super::{
    sim::{AntiRollBar, Differential, Engine, Lean, Transmission, Vehicle, Wheel, WheelTester},
    spec::{Spec, WheelSpec},
};

/// How a seat has its sitter sit: `character.Anim`'s Drive and Ride.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeatAnim {
    #[default]
    Idle,
    Drive,
    Ride,
}

/// How a kind of vehicle drives.
#[derive(Debug, Clone, PartialEq)]
pub struct Handling {
    pub mass: f32,       // kg, all up
    pub wheel_mass: f32, // kg, each, for its inertia
    pub torque: f32,     // the engine's most, Nm
    pub min_rpm: f32,
    pub max_rpm: f32,
    pub shift_up: f32,   // rpm
    pub shift_down: f32, // rpm
    pub gears: Vec<f32>,
    pub reverse: f32,
    pub final_drive: f32, // the differentials' ratio
    pub bump: f32,        // m the suspension can rise from rest
    pub frequency: f32,   // the suspension's spring, Hz
    pub damping: f32,     // 0..1
    pub steer: f32,       // radians, front wheels at full lock
    /// Radians, steering wheels behind the middle: negative turns them
    /// against the front.
    pub rear_steer: f32,
    pub steer_fade: f32, // m/s by which steering is down to half
    pub brake: f32,      // Nm a wheel
    pub hand_brake: f32, // Nm a wheel that has one
    /// An anti-roll bar's stiffness on each axle (N/m), 0 for none, which
    /// none of them has: Jolt's bars, as stiff as would keep these bodies
    /// flat, pump them into rocking side to side on uneven ground until a
    /// wheel's off it.
    pub anti_roll: f32,
    pub grip: f32,
    pub pitch_roll: f32, // radians it can tip before it's held; 0 no limit
    pub lean: Option<Lean>,
    pub caster: f32, // radians the front fork leans back (a bike's)
    pub seat: SeatAnim,
    pub top: f32, // m/s it's limited to; 0 none
}

impl Default for Handling {
    fn default() -> Self {
        Handling {
            mass: 0.0,
            wheel_mass: 0.0,
            torque: 0.0,
            min_rpm: 0.0,
            max_rpm: 0.0,
            shift_up: 0.0,
            shift_down: 0.0,
            gears: Vec::new(),
            reverse: 0.0,
            final_drive: 0.0,
            bump: 0.0,
            frequency: 0.0,
            damping: 0.0,
            steer: 0.0,
            rear_steer: 0.0,
            steer_fade: 0.0,
            brake: 0.0,
            hand_brake: 0.0,
            anti_roll: 0.0,
            grip: 0.0,
            pitch_roll: 0.0,
            lean: None,
            caster: 0.0,
            seat: SeatAnim::Idle,
            top: 0.0,
        }
    }
}

/// The kinds of vehicle, by the name a Spec gives.
pub fn handling(name: &str) -> Option<Handling> {
    let deg = std::f32::consts::PI / 180.0;
    Some(match name {
        // A light four-by-four with long travel: lively, and forgiving.
        "buggy" => Handling {
            mass: 950.0,
            wheel_mass: 25.0,
            torque: 300.0,
            min_rpm: 900.0,
            max_rpm: 6500.0,
            shift_up: 5200.0,
            shift_down: 2400.0,
            gears: vec![2.8, 1.9, 1.4, 1.05, 0.82],
            reverse: 3.0,
            final_drive: 3.2,
            bump: 0.24,
            frequency: 1.35,
            damping: 0.45,
            steer: 0.6,
            steer_fade: 26.0,
            brake: 1600.0,
            hand_brake: 3500.0,
            grip: 1.15,
            pitch_roll: 70.0 * deg,
            seat: SeatAnim::Drive,
            top: 38.0,
            ..default()
        },
        // Two steering wheels in front, one driven behind; narrow, so it's
        // kept from rolling over in turns.
        "trike" => Handling {
            mass: 380.0,
            wheel_mass: 14.0,
            torque: 120.0,
            min_rpm: 900.0,
            max_rpm: 7000.0,
            shift_up: 6000.0,
            shift_down: 2600.0,
            gears: vec![2.6, 1.8, 1.35, 1.05, 0.85],
            reverse: 2.6,
            final_drive: 2.6,
            bump: 0.16,
            frequency: 1.6,
            damping: 0.5,
            steer: 0.5,
            steer_fade: 9.0,
            brake: 700.0,
            hand_brake: 1500.0,
            grip: 0.95,
            pitch_roll: 40.0 * deg,
            seat: SeatAnim::Ride,
            top: 30.0,
            ..default()
        },
        // Two wheels: it leans into turns and holds itself up (Jolt's
        // motorcycle controller).
        "bike" => Handling {
            mass: 230.0,
            wheel_mass: 10.0,
            torque: 70.0,
            min_rpm: 1000.0,
            max_rpm: 8000.0,
            shift_up: 7000.0,
            shift_down: 3000.0,
            gears: vec![2.27, 1.63, 1.3, 1.09, 0.96, 0.88],
            reverse: 2.0,
            final_drive: 2.6,
            bump: 0.14,
            frequency: 1.8,
            damping: 0.6,
            steer: 0.5,
            steer_fade: 14.0,
            brake: 500.0,
            hand_brake: 700.0,
            grip: 1.2,
            lean: Some(Lean {
                max_angle: 45.0 * deg,
                ..default()
            }),
            caster: 30.0 * deg,
            seat: SeatAnim::Ride,
            top: 36.0,
            ..default()
        },
        // A pressurised six-wheeler: heavy and low-geared, its back wheels
        // steering against the front ones to turn tighter.
        "rover" => Handling {
            mass: 3600.0,
            wheel_mass: 45.0,
            torque: 900.0,
            min_rpm: 600.0,
            max_rpm: 3600.0,
            shift_up: 2900.0,
            shift_down: 1400.0,
            gears: vec![3.6, 2.3, 1.5, 1.0],
            reverse: 3.8,
            final_drive: 2.7,
            bump: 0.26,
            frequency: 1.15,
            damping: 0.5,
            steer: 0.5,
            rear_steer: -0.25,
            steer_fade: 14.0,
            brake: 5000.0,
            hand_brake: 8000.0,
            grip: 1.0,
            pitch_roll: 50.0 * deg,
            seat: SeatAnim::Drive,
            top: 22.0,
            ..default()
        },
        // The haulers: a cab-over truck on six or eight driven wheels.
        "truck" => Handling {
            mass: 9500.0,
            wheel_mass: 90.0,
            torque: 1800.0,
            min_rpm: 600.0,
            max_rpm: 3000.0,
            shift_up: 2400.0,
            shift_down: 1200.0,
            gears: vec![4.0, 2.7, 1.8, 1.3, 1.0],
            reverse: 4.2,
            final_drive: 2.4,
            bump: 0.24,
            frequency: 1.2,
            damping: 0.5,
            steer: 0.45,
            steer_fade: 14.0,
            brake: 12000.0,
            hand_brake: 18000.0,
            grip: 1.0,
            pitch_roll: 45.0 * deg,
            seat: SeatAnim::Drive,
            top: 24.0,
            ..default()
        },
        _ => return None,
    })
}

/// Metres of suspension left fully raised.
pub const SUSPENSION_MIN: f32 = 0.05;

pub const GRAVITY: f32 = 9.81;

/// How far a spring of frequency `f` sinks under the weight it carries.
pub fn sag(f: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * f;
    GRAVITY / (w * w)
}

/// Shares the weight of `mass`, centred at `com`, among `wheels`: as evenly
/// as balances it, front to back and side to side.
pub fn loads(wheels: &[WheelSpec], mass: f32, com: Vec3) -> Vec<f32> {
    let n = wheels.len() as f32;
    let (mut mx, mut mz) = (0.0, 0.0);
    for w in wheels {
        mx += w.at[0] / n;
        mz += w.at[2] / n;
    }
    let (mut vx, mut vz) = (0.0, 0.0);
    for w in wheels {
        vx += (w.at[0] - mx) * (w.at[0] - mx);
        vz += (w.at[2] - mz) * (w.at[2] - mz);
    }
    let weight = mass * GRAVITY;
    let (mut a, mut b) = (0.0, 0.0);
    if vz > 1e-4 {
        a = weight * (com.z - mz) / vz;
    }
    if vx > 1e-4 {
        b = weight * (com.x - mx) / vx;
    }
    wheels
        .iter()
        .map(|w| {
            // Never less than a tenth of an even share: a wheel the weight
            // barely reaches still has a spring.
            (weight / n / 10.0).max(weight / n + a * (w.at[2] - mz) + b * (w.at[0] - mx))
        })
        .collect()
}

/// The axis of a wheel's model that faces the vehicle's right: the models
/// are of left wheels, whose outer face (+X) faces left.
pub fn model_right(left: bool) -> Vec3 {
    if left { Vec3::NEG_X } else { Vec3::X }
}

/// The wheeled vehicle for `spec` handled as `h`, its weight centred at
/// `com`.
pub fn build(spec: &Spec, h: &Handling, com: Vec3) -> Vehicle {
    let mut v = Vehicle {
        engine: Engine {
            max_torque: h.torque,
            min_rpm: h.min_rpm,
            max_rpm: h.max_rpm,
        },
        transmission: Transmission {
            gears: h.gears.clone(),
            reverse: h.reverse,
            shift_up_rpm: h.shift_up,
            shift_down_rpm: h.shift_down,
            ..default()
        },
        max_pitch_roll: h.pitch_roll,
        lean: h.lean,
        tester: WheelTester::CastCylinder,
        ..default()
    };
    // The middle, front to back: wheels ahead of it steer as the front, and
    // behind it as the rear.
    let mut mid = 0.0;
    for w in &spec.wheels {
        mid += w.at[2];
    }
    if !spec.wheels.is_empty() {
        mid /= spec.wheels.len() as f32;
    }
    let rest = SUSPENSION_MIN + h.bump;
    let omega = 2.0 * std::f32::consts::PI * h.frequency;
    let load = loads(&spec.wheels, h.mass, com);
    for (i, w) in spec.wheels.iter().enumerate() {
        // Each spring is as stiff as makes its wheel's load sink it by sag:
        // so at rest, every wheel's centre is where the model has it.
        let stiffness = load[i] * omega * omega / GRAVITY;
        let damping = 2.0 * h.damping * (stiffness * load[i] / GRAVITY).sqrt();
        let mut down = Vec3::NEG_Y;
        let front = w.at[2] > mid;
        if h.caster != 0.0 && front {
            // The fork leans back: the wheel goes down and forward along it.
            down = Vec3::new(0.0, -h.caster.cos(), h.caster.sin());
        }
        let mut wheel = Wheel {
            // Attached so that at rest, the spring sunk by its sag, the
            // wheel's centre is where the model has it.
            position: Vec3::from(w.at) - down * rest,
            radius: w.radius,
            width: w.width,
            suspension_min: SUSPENSION_MIN,
            suspension_max: rest + sag(h.frequency),
            stiffness,
            damping_rate: damping,
            brake: h.brake,
            inertia: 0.5 * h.wheel_mass * w.radius * w.radius,
            grip: h.grip,
            suspension_dir: down,
            steering_axis: -down,
            model_right: model_right(w.left),
            ..default()
        };
        if w.steer {
            wheel.max_steer = if front { h.steer } else { h.rear_steer };
        }
        if w.hand_brake {
            wheel.hand_brake = h.hand_brake;
        }
        v.wheels.push(wheel);
    }
    for [left, right] in axles(&spec.wheels) {
        if any_driven(&spec.wheels, &[left, right]) {
            v.differentials.push(Differential {
                left,
                right,
                ratio: h.final_drive,
                ..default()
            });
        }
        if h.anti_roll > 0.0
            && let (Some(left), Some(right)) = (left, right)
        {
            v.anti_roll_bars.push(AntiRollBar {
                left,
                right,
                stiffness: h.anti_roll,
            });
        }
    }
    v
}

/// Pairs the wheels side by side: a left and a right wheel index each,
/// None where a side has none (a bike's, or a trike's back wheel).
pub fn axles(wheels: &[WheelSpec]) -> Vec<[Option<usize>; 2]> {
    let mut out = Vec::new();
    let mut used = vec![false; wheels.len()];
    let side = |w: &WheelSpec| if w.left { 0 } else { 1 };
    for (i, w) in wheels.iter().enumerate() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut pair = [None, None];
        pair[side(w)] = Some(i);
        for (j, o) in wheels.iter().enumerate().skip(i + 1) {
            if !used[j] && o.left != w.left && (o.at[2] - w.at[2]).abs() < 0.25 {
                used[j] = true;
                pair[side(o)] = Some(j);
                break;
            }
        }
        out.push(pair);
    }
    out
}

fn any_driven(wheels: &[WheelSpec], is: &[Option<usize>]) -> bool {
    is.iter().flatten().any(|&i| wheels[i].drive)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(x: f32, z: f32, drive: bool) -> WheelSpec {
        WheelSpec {
            at: [x, 0.38, z],
            radius: 0.38,
            width: 0.28,
            left: x > 0.0,
            steer: z > 0.0,
            drive,
            hand_brake: z < 0.0,
            ..default()
        }
    }

    #[test]
    fn loads_balance_the_weight_about_the_centre_of_mass() {
        let wheels = [
            wheel(0.85, 1.3, true),
            wheel(-0.85, 1.3, true),
            wheel(0.85, -1.3, true),
            wheel(-0.85, -1.3, true),
        ];
        let load = loads(&wheels, 950.0, Vec3::new(0.0, 0.38, 0.0));
        let total: f32 = load.iter().sum();
        assert!((total - 950.0 * GRAVITY).abs() < 1e-2, "{load:?}");
        assert!(load.iter().all(|l| (l - load[0]).abs() < 1e-3), "{load:?}");
        // Weight forward loads the front wheels more.
        let load = loads(&wheels, 950.0, Vec3::new(0.0, 0.38, 0.5));
        assert!(load[0] > load[2], "{load:?}");
        assert!((load.iter().sum::<f32>() - 950.0 * GRAVITY).abs() < 1e-2);
    }

    #[test]
    fn axles_pair_wheels_side_by_side() {
        let trike = [
            wheel(0.44, 0.85, false),
            wheel(-0.44, 0.85, false),
            wheel(0.0, -0.95, true),
        ];
        assert_eq!(axles(&trike), vec![[Some(0), Some(1)], [None, Some(2)]]);
    }

    #[test]
    fn built_vehicle_hangs_its_wheels_where_the_model_has_them() {
        let spec = Spec {
            handling: "buggy".into(),
            wheels: vec![
                wheel(0.85, 1.3, true),
                wheel(-0.85, 1.3, true),
                wheel(0.85, -1.3, true),
                wheel(-0.85, -1.3, true),
            ],
            ..default()
        };
        let h = handling("buggy").unwrap();
        let v = build(&spec, &h, Vec3::new(0.0, 0.38, 0.0));
        assert_eq!(v.wheels.len(), 4);
        assert_eq!(v.differentials.len(), 2);
        for (w, s) in v.wheels.iter().zip(&spec.wheels) {
            // At rest the spring sinks by its sag: the wheel's centre is At.
            let at_rest = w.position + w.suspension_dir * (w.suspension_max - sag(h.frequency));
            assert!(at_rest.distance(Vec3::from(s.at)) < 1e-5, "{at_rest}");
            assert!((w.stiffness * sag(h.frequency) - h.mass * GRAVITY / 4.0).abs() < 1e-2);
        }
        assert_eq!(v.wheels[0].max_steer, h.steer);
        assert_eq!(v.wheels[2].max_steer, 0.0);
        assert_eq!(v.wheels[2].hand_brake, h.hand_brake);
        assert_eq!(v.wheels[0].hand_brake, 0.0);
    }
}
