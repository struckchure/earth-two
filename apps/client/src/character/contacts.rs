//! The capsule moves the character; these contacts keep the animated limbs
//! outside nearby static walls without turning the skeleton into a
//! ragdoll. Port of `character/contacts.go`'s geometry: the animation
//! module samples the blended pose, so transitions and clothing obey the
//! same contacts, and hands it to [`fit_pose`] with the ray casts.

use bevy::prelude::*;
use std::f32::consts::PI;

use super::{RayHit, direction, horizontal};

/// A skeleton bone: its name and parent (-1 for a root), as raylib's
/// BoneInfo gives them.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneInfo {
    pub name: String,
    pub parent: i32,
}

/// A bone's pose, in model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for BoneTransform {
    fn default() -> Self {
        BoneTransform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LimbContact {
    pub upper: usize,
    pub middle: usize,
    pub end: usize,
    pub tips: Vec<ContactPoint>,
    pub probes: Vec<ContactPoint>,
    pub radius: f32,
    pub leg: bool,
    pub offset: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactPoint {
    pub bone: usize,
    pub radius: f32,
}

/// The contacts of one body, kept between frames: the limbs, the torso
/// probes, and the clearances still being released.
#[derive(Clone, Debug, Default)]
pub struct ContactRig {
    pub limbs: Vec<LimbContact>,
    pub torso: Vec<ContactPoint>,
    /// World-space posture clearance, released smoothly.
    pub offset: Vec3,
    pub position: Vec3,
    pub positioned: bool,
    planes: Vec<ContactPlane>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactPlane {
    pub point: Vec3,
    pub normal: Vec3,
}

/// Separates a ladder's own rails and rungs, 37 cm ahead of the climb
/// axis, from the backing wall behind them (65 cm or more).
pub const LADDER_DEPTH: f32 = 0.5;

/// The contacts of a skeleton with the MakeHuman game-engine rig's bone
/// names.
pub fn make_contact_rig(bones: &[BoneInfo]) -> ContactRig {
    let mut r = ContactRig::default();
    let index = |name: &str| bones.iter().position(|b| b.name == name);
    for name in ["pelvis", "spine_02", "spine_03", "neck_01", "head"] {
        if let Some(bone) = index(name) {
            let radius = match name {
                "head" => 0.18,
                "neck_01" => 0.12,
                _ => 0.21,
            };
            r.torso.push(ContactPoint { bone, radius });
        }
    }
    for side in ["l", "r"] {
        for leg in [false, true] {
            let (names, radius, points): ([String; 3], f32, Vec<(String, f32)>) = if leg {
                (
                    [
                        format!("thigh_{side}"),
                        format!("calf_{side}"),
                        format!("foot_{side}"),
                    ],
                    // Includes the thigh/knee surface and fitted trousers.
                    0.14,
                    vec![
                        (format!("foot_{side}"), 0.085),
                        (format!("ball_{side}"), 0.12),
                    ],
                )
            } else {
                (
                    [
                        format!("upperarm_{side}"),
                        format!("lowerarm_{side}"),
                        format!("hand_{side}"),
                    ],
                    0.085,
                    vec![
                        (format!("hand_{side}"), 0.055),
                        (format!("middle_01_{side}"), 0.06),
                        (format!("middle_03_{side}"), 0.04),
                        (format!("index_03_{side}"), 0.04),
                        (format!("thumb_03_{side}"), 0.035),
                    ],
                )
            };
            let (Some(upper), Some(middle), Some(end)) =
                (index(&names[0]), index(&names[1]), index(&names[2]))
            else {
                continue;
            };
            let mut limb = LimbContact {
                upper,
                middle,
                end,
                radius,
                leg,
                ..Default::default()
            };
            // Skeleton order is stable; contacts must be too.
            for (i, bone) in bones.iter().enumerate() {
                if let Some((_, size)) = points.iter().find(|(name, _)| *name == bone.name) {
                    limb.tips.push(ContactPoint {
                        bone: i,
                        radius: *size,
                    });
                }
            }
            limb.probes.push(ContactPoint {
                bone: limb.middle,
                radius: limb.radius,
            });
            limb.probes.extend(limb.tips.iter().copied());
            r.limbs.push(limb);
        }
    }
    r
}

/// What a pose is fitted against: the traversal state that decides which
/// surfaces count.
#[derive(Clone, Copy, Debug, Default)]
pub struct FitContext {
    /// On a ladder, climbing or getting on: the pose is authored against
    /// the rails and rungs, so only the backing wall corrects it.
    pub climbing: bool,
    /// The ladder's bottom and facing, while climbing.
    pub ladder: Option<(Vec3, Vec3)>,
    /// Climbing off: the clip is authored against the ladder and its
    /// landing from start to finish, and correcting it only makes it
    /// twitch.
    pub exiting: bool,
}

/// Fits `pose` (model space, `matrix` taking it to the world) to the
/// static world around it, as `fitPoseToWorld` did for each body: the
/// torso first, then each limb against the walls its probes find. `cast`
/// is a ray cast excluding the character's own body, and `solid` whether a
/// hit is a wall to keep out of. Returns whether anything was corrected;
/// if not, the caller plays the sampled pose as it was.
#[allow(clippy::too_many_arguments)]
pub fn fit_pose(
    rig: &mut ContactRig,
    pose: &mut [BoneTransform],
    bones: &[BoneInfo],
    matrix: Mat4,
    root_position: Vec3,
    context: FitContext,
    cast: &dyn Fn(Vec3, Vec3, f32) -> Option<RayHit>,
    solid: &dyn Fn(&RayHit) -> bool,
    dt: f32,
) -> bool {
    if rig.positioned && root_position.distance_squared(rig.position) > 1.0 {
        rig.offset = Vec3::ZERO;
        for limb in &mut rig.limbs {
            limb.offset = Vec3::ZERO;
        }
    }
    rig.position = root_position;
    rig.positioned = true;
    if pose.is_empty() {
        return false;
    }
    if context.exiting {
        rig.offset = Vec3::ZERO;
        for limb in &mut rig.limbs {
            limb.offset = Vec3::ZERO;
        }
        return false;
    }
    let inverse = matrix.inverse();
    let origin = matrix.transform_point3(Vec3::ZERO);
    let scale = matrix.x_axis.truncate().length();
    let climbing = context.climbing;
    let solid_here = |hit: &RayHit| -> bool {
        if !solid(hit) {
            return false;
        }
        match (climbing, context.ladder) {
            // A knee between the rails is not a knee through a wall.
            (true, Some((bottom, facing))) => {
                (hit.point - bottom).dot(direction(horizontal(facing))) >= LADDER_DEPTH
            }
            _ => true,
        }
    };
    let mut corrected = false;
    if climbing {
        rig.offset = Vec3::ZERO;
    } else {
        corrected = fit_torso(
            rig,
            pose,
            cast,
            &solid_here,
            matrix,
            inverse,
            origin,
            scale,
            dt,
        );
    }
    let mut planes = std::mem::take(&mut rig.planes);
    for limb in &mut rig.limbs {
        // Authored rail grips and rung feet remain exact while attached.
        // Knees still fold away from the ladder's backing wall.
        if climbing && !limb.leg {
            limb.offset = Vec3::ZERO;
            continue;
        }
        planes.clear();
        for probe in &limb.probes {
            let point = matrix.transform_point3(pose[probe.bone].translation);
            let mut from = origin;
            from.y = point.y;
            let delta = horizontal(point - from);
            let length = delta.length();
            if length < 0.01 {
                continue;
            }
            let dir = delta / length;
            // Sampling above/below a rung finds the backing wall instead of
            // treating the intended foot contact as a reason to move off it.
            let offsets: &[f32] = if climbing { &[-0.10, 0.10] } else { &[0.0] };
            for offset in offsets {
                let mut start = from;
                start.y += offset;
                let Some(hit) = cast(start, dir, length + 4.0 * probe.radius * scale + 0.04) else {
                    continue;
                };
                if !solid_here(&hit) {
                    continue;
                }
                let p_point = inverse.transform_point3(hit.point);
                let normal_end = inverse.transform_point3(hit.point + hit.normal);
                let plane = ContactPlane {
                    point: p_point,
                    normal: (normal_end - p_point).normalize(),
                };
                let duplicate = planes.iter().any(|other| {
                    plane.normal.dot(other.normal) > 0.995
                        && (plane.point - other.point).dot(other.normal).abs() < 0.02
                });
                if !duplicate {
                    planes.push(plane);
                }
            }
        }
        corrected = fit_limb(pose, bones, limb, &planes, climbing, dt) || corrected;
    }
    rig.planes = planes;
    corrected
}

/// A leaning jump can put the head/chest beyond the controller. Reserve
/// that space for the entire posture before bending individual limbs; this
/// keeps the spine intact and releases the small offset smoothly after
/// leaving.
#[allow(clippy::too_many_arguments)]
pub fn fit_torso(
    rig: &mut ContactRig,
    pose: &mut [BoneTransform],
    cast: &dyn Fn(Vec3, Vec3, f32) -> Option<RayHit>,
    solid: &dyn Fn(&RayHit) -> bool,
    matrix: Mat4,
    inverse: Mat4,
    origin: Vec3,
    scale: f32,
    dt: f32,
) -> bool {
    let mut offset = rig.offset * (-18.0 * dt).exp();
    for _ in 0..2 {
        for probe in &rig.torso {
            let mut point = matrix.transform_point3(pose[probe.bone].translation) + offset;
            let mut from = origin;
            from.y = point.y;
            let delta = horizontal(point - from);
            let length = delta.length();
            if length < 0.005 {
                continue;
            }
            let dir = delta / length;
            let radius = probe.radius * scale;
            for height in [0.0, -radius * 0.6, radius * 0.6] {
                let mut start = from;
                start.y += height;
                let Some(hit) = cast(start, dir, length + 4.0 * radius) else {
                    continue;
                };
                if !solid(&hit) {
                    continue;
                }
                let depth = radius + 0.015 - (point - hit.point).dot(hit.normal);
                if depth > 0.0 {
                    let push = hit.normal * depth;
                    offset += push;
                    point += push;
                }
            }
        }
    }
    if offset.length_squared() < 0.000001 {
        rig.offset = Vec3::ZERO;
        return false;
    }
    rig.offset = offset;
    let local = inverse.transform_point3(origin + offset) - inverse.transform_point3(origin);
    for bone in pose.iter_mut() {
        bone.translation += local;
    }
    true
}

pub fn plane_distance(point: Vec3, plane: ContactPlane) -> f32 {
    (point - plane.point).dot(plane.normal)
}

/// Bends one limb out of the planes it penetrates, keeping its lengths,
/// and releases the bend smoothly once it's clear. With `keep_foot` the
/// end stays planted (a foot on a rung) and only the knee folds.
pub fn fit_limb(
    pose: &mut [BoneTransform],
    bones: &[BoneInfo],
    limb: &mut LimbContact,
    planes: &[ContactPlane],
    keep_foot: bool,
    dt: f32,
) -> bool {
    let original = pose[limb.end].translation;
    let mut target = original + limb.offset * (-18.0 * dt).exp();
    if keep_foot {
        target = original;
    }
    if !keep_foot {
        // Project the entire palm/sole envelope, not only the wrist or
        // ankle.
        for _ in 0..4 {
            for plane in planes {
                let shift = target - original;
                let mut depth = 0.0f32;
                for tip in &limb.tips {
                    let point = pose[tip.bone].translation + shift;
                    depth = depth.max(tip.radius + 0.008 - plane_distance(point, *plane));
                }
                if depth > 0.0 {
                    target += plane.normal * depth;
                }
            }
        }
    }
    let (upper, mut middle) = (pose[limb.upper].translation, pose[limb.middle].translation);
    let mut blocked = target.distance_squared(original) > 0.000001;
    for plane in planes {
        blocked = blocked || plane_distance(middle, *plane) < limb.radius + 0.008;
    }
    if !blocked {
        limb.offset = Vec3::ZERO;
        return false;
    }
    let (elbow, end) = solve_limb(upper, middle, original, target, planes, limb.radius + 0.008);
    let orientation = pose[limb.end].rotation;
    rotate_branch(
        pose,
        bones,
        limb.upper,
        limb_rotation(middle - upper, elbow - upper),
    );
    middle = pose[limb.middle].translation;
    rotate_branch(
        pose,
        bones,
        limb.middle,
        limb_rotation(pose[limb.end].translation - middle, end - middle),
    );
    let fix = orientation * pose[limb.end].rotation.inverse();
    rotate_branch(pose, bones, limb.end, fix);
    limb.offset = pose[limb.end].translation - original;
    true
}

/// An analytic two-bone solve preserves lengths. Search its elbow/knee
/// circle for a bend outside the walls, preferring the authored pose over
/// a pole flip.
pub fn solve_limb(
    start: Vec3,
    middle: Vec3,
    end: Vec3,
    target: Vec3,
    planes: &[ContactPlane],
    radius: f32,
) -> (Vec3, Vec3) {
    let (a, b) = (start.distance(middle), middle.distance(end));
    let delta = target - start;
    let mut distance = delta.length();
    if a < 0.001 || b < 0.001 || distance < 0.001 {
        return (middle, end);
    }
    let dir = delta / distance;
    distance = distance.max((a - b).abs() + 0.0001).min(a + b - 0.0001);
    let target = start + dir * distance;
    let along = (a * a - b * b + distance * distance) / (2.0 * distance);
    let height = (a * a - along * along).max(0.0).sqrt();
    let center = start + dir * along;
    let mut bend = middle - center;
    bend -= dir * bend.dot(dir);
    if bend.length_squared() < 0.000001 {
        bend = dir.cross(Vec3::Z);
        if bend.length_squared() < 0.000001 {
            bend = dir.cross(Vec3::X);
        }
    }
    let bend = bend.normalize();
    let across = dir.cross(bend);
    let mut best = middle;
    let mut score = f32::MAX;
    for i in 0..48 {
        let angle = i as f32 * 2.0 * PI / 48.0;
        let radial = bend * angle.cos() + across * angle.sin();
        let candidate = center + radial * height;
        let mut cost = candidate.distance_squared(middle);
        for plane in planes {
            let depth = (radius - plane_distance(candidate, *plane)).max(0.0);
            cost += depth * depth * 10000.0;
        }
        if cost < score {
            best = candidate;
            score = cost;
        }
    }
    (best, target)
}

/// Turns the bone `root` and everything under it by `rotation` about the
/// root's position.
pub fn rotate_branch(pose: &mut [BoneTransform], bones: &[BoneInfo], root: usize, rotation: Quat) {
    let rotation = rotation.normalize();
    let pivot = pose[root].translation;
    for (i, bone) in pose.iter_mut().enumerate() {
        let mut current = i as i32;
        while current != root as i32 && current >= 0 && (current as usize) < bones.len() {
            let parent = bones[current as usize].parent;
            if parent == current {
                current = -1;
                break;
            }
            current = parent;
        }
        if current != root as i32 {
            continue;
        }
        bone.translation = pivot + rotation * (bone.translation - pivot);
        bone.rotation = rotation * bone.rotation;
    }
}

/// The rotation taking the direction `from` to `to`.
pub fn limb_rotation(from: Vec3, to: Vec3) -> Quat {
    if from.length_squared() < 0.000001 || to.length_squared() < 0.000001 {
        return Quat::IDENTITY;
    }
    let (from, to) = (from.normalize(), to.normalize());
    if from.dot(to) < -0.9999 {
        let mut axis = from.cross(Vec3::Y);
        if axis.length_squared() < 0.000001 {
            axis = from.cross(Vec3::X);
        }
        return Quat::from_axis_angle(axis.normalize(), PI);
    }
    Quat::from_rotation_arc(from, to)
}
