//! Skeletons, keyframed poses and bone attachments: illusion's
//! render/bones.go and the pose parts of animation.go and pose.go, on data
//! rather than raylib's C structs. The viewer's bones are Bevy's joint
//! entities; this path serves the headless tests and the cloth solver, which
//! poses meshes itself.

use bevy::prelude::*;
use std::sync::Arc;

use super::player::{AnimationPlayer, ClipLibrary};

/// Skeleton is a model's bones: names, parents (-1 for a root) and the bind
/// pose, each in model space (as raylib's ModelSkeleton holds them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    pub names: Vec<String>,
    pub parents: Vec<i32>,
    pub bind: Vec<Transform>,
}

impl Skeleton {
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// index finds the bone called name.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}

/// PoseClip is one clip's keyframes: for each keyframe, a model-space
/// transform for each bone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoseClip {
    pub name: String,
    pub keyframes: Vec<Vec<Transform>>,
}

impl PoseClip {
    pub fn keyframe_count(&self) -> usize {
        self.keyframes.len()
    }

    pub fn bone_count(&self) -> usize {
        self.keyframes.first().map_or(0, Vec::len)
    }

    pub fn frame_pose(&self, frame: usize, bone: usize) -> Transform {
        self.keyframes[frame][bone]
    }
}

/// PoseClips are a model's keyframed clips: raylib's ModelAnimations, which
/// it samples at FRAME_RATE keyframes a second.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoseClips {
    pub clips: Vec<PoseClip>,
    /// Keyframes per second; 0 means 60, the rate raylib samples glTF at.
    pub frame_rate: f32,
}

impl PoseClips {
    pub fn clip(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }

    fn frame_rate(&self) -> f32 {
        if self.frame_rate <= 0.0 {
            60.0
        } else {
            self.frame_rate
        }
    }

    /// duration returns the length of clip i in seconds.
    pub fn duration(&self, i: usize) -> f32 {
        self.clips[i].keyframe_count().saturating_sub(1) as f32 / self.frame_rate()
    }

    /// library is the clip clock's view of these clips.
    pub fn library(&self) -> ClipLibrary {
        ClipLibrary::new(
            (0..self.clips.len()).map(|i| (self.clips[i].name.clone(), self.duration(i))),
        )
    }

    /// frame returns the (fractional) keyframe of clip i after t seconds.
    /// Looping clips wrap at their last keyframe, which is equal to the
    /// first, so nothing interpolates from the last keyframe back to the
    /// first. Clips played once hold their end pose (see end).
    pub fn frame(&self, i: usize, t: f32, once: bool) -> f32 {
        let last = self.clips[i].keyframe_count().saturating_sub(1) as f32;
        if last == 0.0 {
            return 0.0;
        }
        let f = t * self.frame_rate();
        if once {
            return f.min(self.end(i));
        }
        f % last
    }

    /// end is the keyframe clip i holds once it's played through: its last,
    /// or the one before when the last is a copy of the first. A glTF clip's
    /// final keyframe is sampled past its last key, which gives the first
    /// pose again: right for a loop, but a clip that ends somewhere else
    /// would snap back to where it started.
    pub fn end(&self, i: usize) -> f32 {
        let c = &self.clips[i];
        let last = c.keyframe_count() as i64 - 1;
        if last < 2 || c.bone_count() == 0 {
            return last.max(0) as f32;
        }
        let wrapped = (0..c.bone_count())
            .all(|bone| c.frame_pose(last as usize, bone) == c.frame_pose(0, bone));
        if wrapped {
            (last - 1) as f32
        } else {
            last as f32
        }
    }

    /// fits reports whether clip i can pose skeleton.
    pub fn fits(&self, skeleton: &Skeleton, i: usize) -> bool {
        self.clips[i].bone_count() == skeleton.len() && self.clips[i].keyframe_count() > 0
    }
}

/// sample interpolates a bone's pose at a fractional frame. raylib's
/// UpdateModelAnimation splits the frame before wrapping it, while
/// UpdateModelAnimationEx wraps first; with frames inside the clip, as here,
/// both agree.
pub fn sample(anim: &PoseClip, frame: f32, bone: usize, ex: bool) -> Transform {
    let n = anim.keyframe_count();
    let mut current = frame as usize;
    if ex {
        current %= n;
    }
    let blend = (frame - current as f32).clamp(0.0, 1.0);
    current %= n;
    let next = (current + 1) % n;
    mix(
        anim.frame_pose(current, bone),
        anim.frame_pose(next, bone),
        blend,
    )
}

pub fn mix(a: Transform, b: Transform, t: f32) -> Transform {
    Transform {
        translation: a.translation.lerp(b.translation, t),
        rotation: a.rotation.slerp(b.rotation, t),
        scale: a.scale.lerp(b.scale, t),
    }
}

/// pose_matrix is a pose as raylib builds bone matrices: scale, rotate,
/// move.
pub fn pose_matrix(t: Transform) -> Mat4 {
    t.to_matrix()
}

/// clip_bone_pose is a bone's model-space pose for the player's current
/// state: the current clip, crossfaded from the previous one.
pub fn clip_bone_pose(
    skeleton: &Skeleton,
    p: &AnimationPlayer,
    a: &PoseClips,
    bone: usize,
) -> Option<Transform> {
    let cur = a.clip(p.clip())?;
    if !a.fits(skeleton, cur) {
        return None;
    }
    let frame = a.frame(cur, p.time(), p.once());
    if let Some(prev) = p.previous()
        && let Some(pi) = a.clip(&prev.clip)
        && a.fits(skeleton, pi)
    {
        // As UpdateModelAnimationEx: blend 0 is the previous clip.
        let from = sample(&a.clips[pi], a.frame(pi, prev.time, prev.once), bone, true);
        let to = sample(&a.clips[cur], frame, bone, true);
        return Some(mix(from, to, p.blend()));
    }
    Some(sample(&a.clips[cur], frame, bone, false))
}

/// sample_pose samples every bone's pose from the clips and their current
/// crossfade, ignoring any override.
pub fn sample_pose(
    skeleton: &Skeleton,
    p: &AnimationPlayer,
    a: &PoseClips,
) -> Option<Vec<Transform>> {
    let cur = a.clip(p.clip())?;
    if !a.fits(skeleton, cur) {
        return None;
    }
    Some(
        (0..skeleton.len())
            .map(|i| clip_bone_pose(skeleton, p, a, i).unwrap_or_default())
            .collect(),
    )
}

/// bone_pose is a bone's model-space pose: the override's if one is set,
/// else the clips'.
pub fn bone_pose(
    skeleton: &Skeleton,
    p: &AnimationPlayer,
    a: &PoseClips,
    pose_override: Option<&[Transform]>,
    bone: usize,
) -> Option<Transform> {
    if let Some(pose) = pose_override
        && pose.len() == skeleton.len()
        && bone < pose.len()
    {
        return Some(pose[bone]);
    }
    clip_bone_pose(skeleton, p, a, bone)
}

/// bone_matrices are the matrices that take the bind pose to pose, one a
/// bone: inverse bind, then pose.
pub fn bone_matrices(skeleton: &Skeleton, pose: &[Transform]) -> Vec<Mat4> {
    skeleton
        .bind
        .iter()
        .zip(pose)
        .map(|(bind, pose)| pose_matrix(*pose) * pose_matrix(*bind).inverse())
        .collect()
}

/// PoseSource is a model's skeleton and clips on the entity that draws it,
/// for the headless pose path: bone attachments and the cloth solver read
/// it with the entity's AnimationPlayer.
#[derive(Component, Clone, Debug)]
pub struct PoseSource {
    pub skeleton: Arc<Skeleton>,
    pub clips: Arc<PoseClips>,
    /// The model's own transform (raylib's Model.Transform).
    pub transform: Mat4,
}

/// PoseOverride replaces the sampled skeleton in model space: one transform
/// per bone, set after the clocks advance and before attachments and
/// drawing. The seat, ride and contact systems write it.
#[derive(Component, Clone, Debug, Default)]
pub struct PoseOverride(pub Vec<Transform>);

/// BoneAttachment keeps an entity on a bone of its parent's model,
/// following the parent's AnimationPlayer (or the bind pose without one):
/// every frame, before transforms propagate, it sets the entity's Transform
/// to the bone's pose, then offset.
#[derive(Component, Clone, Debug)]
pub struct BoneAttachment {
    /// The bone's name in the model file.
    pub bone: String,
    /// Places the entity relative to the bone; identity for none.
    pub offset: Transform,
}

/// attachment_transform is where a bone attachment goes: the bone's pose in
/// the model's transform, then the offset.
pub fn attachment_transform(pose: Transform, model: Mat4, offset: Transform) -> Transform {
    let mut m = model * pose_matrix(pose);
    if offset != Transform::IDENTITY {
        m *= pose_matrix(offset);
    }
    Transform::from_matrix(m)
}

/// attach_to_bones moves bone attachments to their bones' current poses.
pub fn attach_to_bones(
    mut q: Query<(&ChildOf, &BoneAttachment, &mut Transform)>,
    models: Query<(&PoseSource, Option<&AnimationPlayer>, Option<&PoseOverride>)>,
) {
    for (parent, at, mut tr) in &mut q {
        let Ok((source, player, pose_override)) = models.get(parent.parent()) else {
            continue;
        };
        let Some(index) = source.skeleton.index(&at.bone) else {
            continue;
        };
        let mut pose = source.skeleton.bind[index];
        if let Some(p) = player
            && let Some(animated) = bone_pose(
                &source.skeleton,
                p,
                &source.clips,
                pose_override.map(|o| o.0.as_slice()),
                index,
            )
        {
            pose = animated;
        }
        *tr = attachment_transform(pose, source.transform, at.offset);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A two-bone skeleton in model space: Root at the origin and Tip one
    /// unit up, in a model shifted two units along X.
    pub(crate) fn test_skeleton() -> (Skeleton, Mat4) {
        (
            Skeleton {
                names: vec!["Root".into(), "Tip".into()],
                parents: vec![-1, 0],
                bind: vec![Transform::IDENTITY, Transform::from_xyz(0.0, 1.0, 0.0)],
            },
            Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        )
    }

    /// pose_clip makes a clip whose Tip rotates about Z through the given
    /// angles, one keyframe each (Root stays put).
    pub(crate) fn pose_clip(name: &str, degrees: &[f32]) -> PoseClip {
        PoseClip {
            name: name.into(),
            keyframes: degrees
                .iter()
                .map(|d| {
                    vec![
                        Transform::IDENTITY,
                        Transform::from_xyz(0.0, 1.0, 0.0)
                            .with_rotation(Quat::from_rotation_z(d.to_radians())),
                    ]
                })
                .collect(),
        }
    }

    pub(crate) fn z_angle(q: Quat) -> f32 {
        2.0 * q.z.atan2(q.w) * 180.0 / std::f32::consts::PI
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn clips() -> PoseClips {
        PoseClips {
            clips: vec![
                pose_clip("Bend", &[0.0, 90.0, 0.0]),
                pose_clip("Rest", &[0.0, 0.0]),
                pose_clip("Up", &[90.0, 90.0]),
            ],
            frame_rate: 60.0,
        }
    }

    // render/bones_test.go TestBoneAttachmentBindPose.
    #[test]
    fn attachment_bind_pose() {
        let (sk, model) = test_skeleton();
        let tr = attachment_transform(sk.bind[1], model, Transform::IDENTITY);
        assert!(
            tr.translation.distance(Vec3::new(2.0, 1.0, 0.0)) < 1e-3
                && near(z_angle(tr.rotation), 0.0)
        );
    }

    // TestBoneAttachmentFollowsAnimation.
    #[test]
    fn attachment_follows_animation() {
        let (sk, model) = test_skeleton();
        let a = clips();
        let mut p = AnimationPlayer::default();
        p.play("Bend");
        p.seek(1.0 / 60.0); // keyframe 1: the tip turned 90° about Z
        let pose = bone_pose(&sk, &p, &a, None, 1).unwrap();
        let tr = attachment_transform(pose, model, Transform::from_xyz(0.0, 0.5, 0.0));
        // The offset is in bone space: half a unit along the tip's own +Y,
        // which the 90° turn points along -X.
        assert!(
            tr.translation.distance(Vec3::new(1.5, 1.0, 0.0)) < 1e-3
                && near(z_angle(tr.rotation), 90.0),
            "{tr:?}"
        );
        p.seek(1.5 / 60.0); // halfway from keyframe 1 back to 0
        let pose = bone_pose(&sk, &p, &a, None, 1).unwrap();
        assert!(near(z_angle(pose.rotation), 45.0));
    }

    // TestBoneAttachmentCrossfades.
    #[test]
    fn attachment_crossfades() {
        let (sk, _) = test_skeleton();
        let a = clips();
        let mut p = AnimationPlayer::default();
        p.play("Rest");
        p.play("Up").fade_in(1.0);
        let lib = a.library();
        for _ in 0..30 {
            p.advance(&lib, 1.0 / 60.0);
        }
        let pose = bone_pose(&sk, &p, &a, None, 1).unwrap();
        assert!(
            near(z_angle(pose.rotation), 45.0),
            "{}",
            z_angle(pose.rotation)
        );
    }

    // TestBoneAttachmentUnknownBone: handled by attach_to_bones leaving the
    // transform alone, which the headless app test covers.
    #[test]
    fn unknown_bone_has_no_index() {
        let (sk, _) = test_skeleton();
        assert_eq!(sk.index("Tail"), None);
    }

    // render/animation_test.go TestAnimationOnceHoldsEndPoseOfWrappedClip.
    #[test]
    fn once_holds_end_pose_of_wrapped_clip() {
        let poses = |ys: &[f32]| PoseClip {
            name: "Climb".into(),
            keyframes: ys
                .iter()
                .map(|y| vec![Transform::from_xyz(0.0, *y, 0.0)])
                .collect(),
        };
        let a = PoseClips {
            clips: vec![
                poses(&[0.0, 1.0, 2.0, 3.0, 0.0]),
                poses(&[0.0, 1.0, 2.0, 3.0, 4.0]),
            ],
            frame_rate: 0.0,
        };
        for (i, want) in [3.0, 4.0].into_iter().enumerate() {
            let f = a.frame(i, 10.0, true);
            let y = sample(&a.clips[i], f, 0, true).translation.y;
            assert_eq!(y, want, "clip {i} held at frame {f}");
        }
        assert!(near(a.frame(0, 4.5 / 60.0, false), 0.5));
    }

    // TestAnimationLoops' frame check and TestAnimationOnceFinishesAndHolds'
    // held frame.
    #[test]
    fn frames_wrap_and_hold() {
        let frames = |n: usize| PoseClip {
            name: "c".into(),
            keyframes: vec![vec![]; n],
        };
        let a = PoseClips {
            clips: vec![frames(61), frames(31)],
            frame_rate: 0.0,
        };
        assert!(near(a.frame(0, 0.25, false), 15.0));
        assert_eq!(a.frame(1, 0.5, true), 30.0);
    }

    // render/pose_override_test.go TestPoseOverrideMatchesClothAndKeepsSamplingClips.
    #[test]
    fn pose_override_matches_cloth_and_keeps_sampling_clips() {
        let (sk, _) = test_skeleton();
        let a = PoseClips {
            clips: vec![
                pose_clip("rest", &[0.0, 0.0]),
                pose_clip("bend", &[90.0, 90.0]),
            ],
            frame_rate: 0.0,
        };
        let mut p = AnimationPlayer::default();
        p.play("rest");
        p.play("bend").fade_in(1.0);
        p.fade = 0.5;
        let sampled = sample_pose(&sk, &p, &a).unwrap();
        assert!(
            sampled.len() == 2 && near(z_angle(sampled[1].rotation), 45.0),
            "sample did not include crossfade"
        );
        let mut pose = sampled.clone();
        pose[1].translation.x = 0.4;
        pose[1].rotation = Quat::IDENTITY;
        let authored = sample_pose(&sk, &p, &a).unwrap();
        assert!(authored[1].translation.x == 0.0 && near(z_angle(authored[1].rotation), 45.0));
        let bone = bone_pose(&sk, &p, &a, Some(&pose), 1).unwrap();
        assert_eq!(bone.translation.x, 0.4, "attachments ignore override");
        // The draw pose and the cloth's bones agree: both come from the
        // same matrices.
        let bones = bone_matrices(&sk, &pose);
        let probe = Vec3::new(0.3, 0.7, -0.4);
        let drawn = pose_matrix(pose[1]) * pose_matrix(sk.bind[1]).inverse();
        assert!(
            bones[1]
                .transform_point3(probe)
                .distance(drawn.transform_point3(probe))
                < 1e-5
        );
        let bone = bone_pose(&sk, &p, &a, None, 1).unwrap();
        assert_eq!(
            bone.translation.x, 0.0,
            "clearing override retained contact"
        );
    }
}
