//! How a piece drives, in the piece's own frame (front toward +Z, standing
//! on y = 0): world.json's "vehicle" for it, as `vehicle/spec.go` reads it.
//! The format lives in `earth_two_world::kit`; these are the derived
//! quantities the Go code computes from it.

use bevy::prelude::*;
pub use earth_two_world::kit::{
    Collider as ChassisBox, HeadlampSpec, SeatSpec, VehicleSpec as Spec, WheelSpec,
};

/// A zero rotation in the manifest means none.
pub fn quat(a: [f32; 4]) -> Quat {
    if a == [0.0; 4] {
        Quat::IDENTITY
    } else {
        Quat::from_array(a)
    }
}

/// Every corner of the chassis boxes, for its convex hull.
pub fn corners(spec: &Spec) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(spec.chassis.len() * 8);
    for b in &spec.chassis {
        let c = Vec3::from(b.center);
        let q = quat(b.rotation);
        for i in 0..8 {
            let mut d = Vec3::from(b.size) / 2.0;
            if i & 1 != 0 {
                d.x = -d.x;
            }
            if i & 2 != 0 {
                d.y = -d.y;
            }
            if i & 4 != 0 {
                d.z = -d.z;
            }
            out.push(c + q * d);
        }
    }
    out
}

/// The box around the chassis: (min, max), zero for no chassis.
pub fn bounds(spec: &Spec) -> (Vec3, Vec3) {
    let cs = corners(spec);
    let Some(first) = cs.first() else {
        return (Vec3::ZERO, Vec3::ZERO);
    };
    cs.iter()
        .fold((*first, *first), |(lo, hi), c| (lo.min(*c), hi.max(*c)))
}

/// How far behind the camera follows.
pub fn chase_distance(spec: &Spec) -> f32 {
    if spec.camera > 0.0 {
        return spec.camera;
    }
    let (lo, hi) = bounds(spec);
    let size = hi - lo;
    (1.2 * size.z + 1.5 * size.y).max(4.5)
}

/// Where the vehicle's weight is: the spec's centre of mass, or, left zero,
/// under the middle of the chassis, as low as the axles: heavy things
/// (batteries, motors, the floor) ride low.
pub fn center_of_mass(spec: &Spec) -> Vec3 {
    let com = Vec3::from(spec.center_of_mass);
    if com != Vec3::ZERO {
        return com;
    }
    let (lo, hi) = bounds(spec);
    let mut com = (lo + hi) * 0.5;
    com.y = 0.0;
    for w in &spec.wheels {
        com.y += w.at[1] / spec.wheels.len() as f32;
    }
    com
}

/// Encloses the chassis about its root in every orientation.
pub fn chassis_radius(spec: Option<&Spec>) -> f32 {
    let Some(spec) = spec else {
        return 3.0;
    };
    let (lo, hi) = bounds(spec);
    Vec3::new((-lo.x).max(hi.x), (-lo.y).max(hi.y), (-lo.z).max(hi.z)).length()
}

/// Lamps on the chassis' front, +Z, for vehicles without authored lamp
/// positions. A narrow bike gets one central lamp; wider vehicles a pair.
pub fn headlamp_mounts(spec: &Spec) -> Vec<HeadlampSpec> {
    if !spec.headlamps.is_empty() {
        return spec.headlamps.clone();
    }
    let (lo, hi) = bounds(spec);
    let width = hi.x - lo.x;
    let y = lo.y + ((hi.y - lo.y) * 0.4).min(0.55);
    let front = Vec3::new((lo.x + hi.x) / 2.0, y.max(0.6), hi.z + 0.08);
    if spec.handling == "bike" {
        let at = Vec3::new(front.x, front.y.max(0.85), front.z);
        return vec![HeadlampSpec {
            at: at.to_array(),
            radius: 0.06,
        }];
    }
    let left = front + Vec3::X * (width * 0.34);
    let right = front - Vec3::X * (width * 0.34);
    vec![
        HeadlampSpec {
            at: left.to_array(),
            radius: 0.06,
        },
        HeadlampSpec {
            at: right.to_array(),
            radius: 0.06,
        },
    ]
}

/// Where a seat's sitter goes on a vehicle posed at `at` turned by `turn`:
/// the seat faces the vehicle's front.
pub fn seat_pose(at: Vec3, turn: Quat, seat: &SeatSpec) -> (Vec3, Quat) {
    (at + turn * Vec3::from(seat.at), turn)
}

/// Points (grips or pegs) in the seat's own frame.
pub fn seat_reach(seat: &SeatSpec, points: &[[f32; 3]]) -> Vec<Vec3> {
    points
        .iter()
        .map(|p| Vec3::from(*p) - Vec3::from(seat.at))
        .collect()
}

/// The heading of a rotation: the angle of its forward (+Z) about +Y.
pub fn yaw_of(q: Quat) -> f32 {
    let f = q * Vec3::Z;
    f.x.atan2(f.z)
}

/// A wheel's model where it's modelled, turned about for the right side.
pub fn parked_wheel(w: &WheelSpec) -> Transform {
    let mut tr = Transform::from_translation(Vec3::from(w.at));
    if !w.left {
        tr.rotation = Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI);
    }
    tr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_follow_the_box_rotation() {
        let spec = Spec {
            chassis: vec![ChassisBox {
                center: [0.0, 0.75, -0.05],
                size: [2.7, 0.6, 0.9],
                // Rounded quaternion from the Go manifest fixture.
                #[allow(clippy::approx_constant)]
                rotation: [0.0, -0.70711, 0.0, 0.70711],
            }],
            ..default()
        };
        let (lo, hi) = bounds(&spec);
        assert!((hi.x - lo.x - 0.9).abs() < 1e-3, "{lo} {hi}");
        assert!((hi.z - lo.z - 2.7).abs() < 1e-3, "{lo} {hi}");
    }

    #[test]
    fn centre_of_mass_sits_at_axle_height_under_the_chassis() {
        let spec = Spec {
            chassis: vec![ChassisBox {
                center: [0.0, 0.95, 0.0],
                size: [1.6, 0.7, 3.6],
                rotation: [0.0; 4],
            }],
            wheels: vec![
                WheelSpec {
                    at: [0.85, 0.38, 1.3],
                    ..default()
                },
                WheelSpec {
                    at: [-0.85, 0.38, -1.3],
                    ..default()
                },
            ],
            ..default()
        };
        let com = center_of_mass(&spec);
        assert!(com.distance(Vec3::new(0.0, 0.38, 0.0)) < 1e-5, "{com}");
    }
}
