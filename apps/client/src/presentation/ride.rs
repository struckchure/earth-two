//! Port of character/ride.go: reshape the driving clip for an astride seat.
use crate::character::contacts::{
    BoneInfo, BoneTransform, limb_rotation, rotate_branch, solve_limb,
};
use bevy::prelude::*;

#[derive(Clone, Debug)]
struct Limb {
    upper: usize,
    middle: usize,
    end: usize,
    side: f32,
    arm: bool,
    fingers: Option<usize>,
    index: Option<usize>,
    pinky: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct RideRig {
    pelvis: Option<usize>,
    spine: Option<usize>,
    limbs: Vec<Limb>,
}

impl RideRig {
    pub fn new(bones: &[BoneInfo]) -> Self {
        let find = |name: &str| bones.iter().position(|b| b.name == name);
        let mut rig = Self {
            pelvis: find("pelvis"),
            spine: find("spine_01"),
            limbs: vec![],
        };
        for (side, sign) in [("l", 1.0), ("r", -1.0)] {
            for arm in [true, false] {
                let names = if arm {
                    ["upperarm_", "lowerarm_", "hand_"]
                } else {
                    ["thigh_", "calf_", "foot_"]
                };
                let [Some(upper), Some(middle), Some(end)] =
                    names.map(|n| find(&format!("{n}{side}")))
                else {
                    continue;
                };
                rig.limbs.push(Limb {
                    upper,
                    middle,
                    end,
                    side: sign,
                    arm,
                    fingers: find(&format!("middle_01_{side}")),
                    index: find(&format!("index_01_{side}")),
                    pinky: find(&format!("pinky_01_{side}")),
                });
            }
        }
        rig
    }

    /// Points are in the seat's frame; seat_to_model includes the body's scale.
    pub fn fit(
        &self,
        pose: &mut [BoneTransform],
        bones: &[BoneInfo],
        seat_to_model: Mat4,
        hands: &[Vec3],
        feet: &[Vec3],
    ) {
        let Some(pelvis) = self.pelvis else { return };
        let up = seat_to_model.transform_vector3(Vec3::Y).normalize();
        let ahead = seat_to_model.transform_vector3(Vec3::Z).normalize();
        let left = up.cross(ahead).normalize();
        let hips = pose[pelvis].translation;
        let reach = |limb: &Limb| {
            let i = usize::from(limb.side < 0.0);
            let (points, fallback, lift) = if limb.arm {
                (hands, Vec3::new(0.3, -0.1, 0.68), 0.035)
            } else {
                (feet, Vec3::new(0.19, -0.47, 0.12), 0.08)
            };
            if let Some(p) = points.get(i) {
                seat_to_model.transform_point3(*p) + up * lift
            } else {
                hips + left * fallback.x * limb.side + up * fallback.y + ahead * fallback.z
            }
        };
        let wrist = |limb: &Limb| reach(limb) - ahead * 0.07;
        if let Some(spine) = self.spine {
            let pivot = pose[spine].translation;
            let mut lean = 55.0_f32.to_radians();
            for degrees in (12..=54).step_by(2) {
                let angle = (degrees as f32).to_radians();
                let turn = Quat::from_axis_angle(-left, -angle);
                let reachable = self.limbs.iter().filter(|l| l.arm).all(|l| {
                    let shoulder = pivot + turn * (pose[l.upper].translation - pivot);
                    let length = pose[l.upper]
                        .translation
                        .distance(pose[l.middle].translation)
                        + pose[l.middle].translation.distance(pose[l.end].translation);
                    shoulder.distance(wrist(l)) <= 0.96 * length
                });
                if reachable {
                    lean = angle;
                    break;
                }
            }
            rotate_branch(pose, bones, spine, Quat::from_axis_angle(-left, -lean));
        }
        for limb in &self.limbs {
            let target = if limb.arm { wrist(limb) } else { reach(limb) };
            let upper = pose[limb.upper].translation;
            let middle = pose[limb.middle].translation;
            let end = pose[limb.end].translation;
            let (joint, reached) = solve_limb(upper, middle, end, target, &[], 0.0);
            let held = pose[limb.end].rotation;
            rotate_branch(
                pose,
                bones,
                limb.upper,
                limb_rotation(middle - upper, joint - upper),
            );
            let middle = pose[limb.middle].translation;
            rotate_branch(
                pose,
                bones,
                limb.middle,
                limb_rotation(pose[limb.end].translation - middle, reached - middle),
            );
            rotate_branch(
                pose,
                bones,
                limb.end,
                held * pose[limb.end].rotation.inverse(),
            );
            if limb.arm {
                grip(pose, bones, limb, ahead, up, left);
            }
        }
    }
}

fn grip(
    pose: &mut [BoneTransform],
    bones: &[BoneInfo],
    limb: &Limb,
    ahead: Vec3,
    up: Vec3,
    left: Vec3,
) {
    let (Some(fingers), Some(index), Some(pinky)) = (limb.fingers, limb.index, limb.pinky) else {
        return;
    };
    let wrist = pose[limb.end].translation;
    let have = frame(
        pose[fingers].translation - wrist,
        pose[index].translation - pose[pinky].translation,
    );
    let want = frame((ahead - up * 0.45).normalize(), left * -limb.side);
    if let (Some(have), Some(want)) = (have, want) {
        rotate_branch(pose, bones, limb.end, want * have.inverse());
    }
}

fn frame(along: Vec3, across: Vec3) -> Option<Quat> {
    if along.length_squared() < 1e-8 {
        return None;
    }
    let x = along.normalize();
    let y = across - x * across.dot(x);
    if y.length_squared() < 1e-8 {
        return None;
    }
    let y = y.normalize();
    Some(Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))))
}

#[cfg(test)]
mod tests {
    use super::*;
    // A metre-scale seated rig: bent elbows and knees, including the palm
    // frame. Same bone names/hierarchy as the shipped MakeHuman skeleton.
    fn fixture() -> (Vec<BoneInfo>, Vec<BoneTransform>) {
        let mut bones = vec![];
        let mut pose = vec![];
        let mut add = |name: String, parent, position| {
            bones.push(BoneInfo { name, parent });
            pose.push(BoneTransform {
                translation: position,
                ..default()
            });
        };
        add("pelvis".into(), -1, Vec3::new(0., 0.7, 0.));
        add("spine_01".into(), 0, Vec3::new(0., 0.9, 0.));
        for (i, (side, sign)) in [("l", 1.), ("r", -1.)].into_iter().enumerate() {
            let base = 2 + i as i32 * 9;
            for (name, parent, p) in [
                ("upperarm", 1, Vec3::new(0.18 * sign, 1.3, 0.)),
                ("lowerarm", base, Vec3::new(0.3 * sign, 1.05, 0.18)),
                ("hand", base + 1, Vec3::new(0.3 * sign, 1.05, 0.48)),
                ("middle_01", base + 2, Vec3::new(0.3 * sign, 1.05, 0.55)),
                ("index_01", base + 2, Vec3::new(0.27 * sign, 1.05, 0.55)),
                ("pinky_01", base + 2, Vec3::new(0.33 * sign, 1.05, 0.55)),
                ("thigh", 0, Vec3::new(0.12 * sign, 0.7, 0.)),
                ("calf", base + 6, Vec3::new(0.18 * sign, 0.7, 0.4)),
                ("foot", base + 7, Vec3::new(0.18 * sign, 0.3, 0.4)),
            ] {
                add(format!("{name}_{side}"), parent, p);
            }
        }
        (bones, pose)
    }

    #[test]
    fn riding_reaches_authored_grips_and_pegs_without_stretching_limbs() {
        let (bones, mut pose) = fixture();
        let original = pose.clone();
        let rig = RideRig::new(&bones);
        let hands = [Vec3::new(0.3, 1.1, 0.6), Vec3::new(-0.3, 1.1, 0.6)];
        let feet = [Vec3::new(0.19, 0.15, 0.12), Vec3::new(-0.19, 0.15, 0.12)];
        rig.fit(&mut pose, &bones, Mat4::IDENTITY, &hands, &feet);
        for limb in &rig.limbs {
            let i = usize::from(limb.side < 0.);
            let target = if limb.arm {
                hands[i] + Vec3::Y * 0.035 - Vec3::Z * 0.07
            } else {
                feet[i] + Vec3::Y * 0.08
            };
            assert!(
                pose[limb.end].translation.distance(target) < 1e-4,
                "{:?} missed {:?}",
                limb,
                target
            );
            for (a, b) in [(limb.upper, limb.middle), (limb.middle, limb.end)] {
                assert!(
                    (pose[a].translation.distance(pose[b].translation)
                        - original[a].translation.distance(original[b].translation))
                    .abs()
                        < 1e-5
                );
            }
            if limb.arm {
                let along = (pose[limb.fingers.unwrap()].translation - pose[limb.end].translation)
                    .normalize();
                assert!(along.dot((Vec3::Z - Vec3::Y * 0.45).normalize()) > 0.9999);
                let across = (pose[limb.index.unwrap()].translation
                    - pose[limb.pinky.unwrap()].translation)
                    .normalize();
                assert!(across.dot(Vec3::X * -limb.side) > 0.9999);
            }
        }
        assert_eq!(pose[0], original[0], "riding moved the hips off the seat");
        assert_ne!(
            pose[1].rotation, original[1].rotation,
            "missing forward lean"
        );
    }

    #[test]
    fn seat_transform_and_missing_targets_keep_pose_finite() {
        let (bones, mut pose) = fixture();
        let rig = RideRig::new(&bones);
        rig.fit(
            &mut pose,
            &bones,
            Mat4::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(1., 2., 3.)),
            &[],
            &[],
        );
        for p in pose {
            assert!(p.translation.is_finite() && p.rotation.is_finite());
        }
        assert!(frame(Vec3::ZERO, Vec3::X).is_none());
        assert!(frame(Vec3::X, Vec3::X).is_none());
    }
}
