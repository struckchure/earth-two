//! Shoes under trouser cuffs: character/footwear.go.

use bevy::math::Vec3;

use super::mesh::ModelMeshes;
use super::verlet::{Capsule, closest_on_segment};

/// footwear_capsules fits the shoe uppers, carried by their own bone
/// weights. Segmenting along the foot keeps the tongue and toe at their own
/// heights; boot shafts weighted to the calf follow the shin independently
/// of the foot.
pub fn footwear_capsules(model: &ModelMeshes) -> Vec<Capsule> {
    let by_bone = model.by_bone();
    let mut out = vec![];
    // Skeleton order makes the fit deterministic when capsules overlap.
    for (bone, points) in by_bone.iter().enumerate() {
        out.extend(fit_shoe_upper(points, bone));
    }
    out
}

pub fn fit_shoe_upper(points: &[Vec3], bone: usize) -> Vec<Capsule> {
    if points.is_empty() {
        return vec![];
    }
    let (mut lo, mut hi) = (points[0].z, points[0].z);
    for p in &points[1..] {
        lo = lo.min(p.z);
        hi = hi.max(p.z);
    }
    const PIECES: usize = 6;
    let mut bins: Vec<Vec<Vec3>> = vec![vec![]; PIECES];
    let span = (hi - lo).max(1e-6);
    for p in points {
        let i = (PIECES - 1).min(((p.z - lo) / span * PIECES as f32) as usize);
        bins[i].push(*p);
    }
    let mut out = vec![];
    for pts in &bins {
        if pts.len() < 3 {
            continue;
        }
        let (mut low, mut high) = (pts[0], pts[0]);
        for p in &pts[1..] {
            low.x = low.x.min(p.x);
            low.z = low.z.min(p.z);
            high.x = high.x.max(p.x);
            high.y = high.y.max(p.y);
            high.z = high.z.max(p.z);
        }
        let r = ((high.x - low.x) / 2.0).max(0.008);
        let centre = Vec3::new((low.x + high.x) / 2.0, high.y - r, 0.0);
        let (mut a, mut b) = (centre, centre);
        a.z = low.z;
        b.z = high.z;
        out.push(Capsule {
            a,
            b,
            radius: r,
            bone_a: bone as i32,
            bone_b: bone as i32,
        });
    }
    out
}

/// cuff_clearance loosens only vertices near shoe uppers. Shorts and bare
/// feet keep their usual fit; cuffs get enough range to clear the selected
/// footwear.
pub fn cuff_clearance(p: Vec3, shoes: &[Capsule]) -> f32 {
    let mut freedom = 0.0f32;
    for shoe in shoes {
        let d = p.distance(closest_on_segment(p, shoe.a, shoe.b)) - shoe.radius;
        if d < 0.025 {
            // Allow the collision projection plus a little settling room.
            freedom = freedom.max(0.10f32.min(0.015f32.max(0.025 - d)));
        }
    }
    freedom
}

#[cfg(test)]
mod tests {
    use super::*;

    // character/footwear_test.go TestShoeUpperFollowsTongueAndToe.
    #[test]
    fn shoe_upper_follows_tongue_and_toe() {
        let mut points = vec![];
        for i in 0..25 {
            let z = i as f32 * 0.01;
            let top = 0.10 - z * 0.2;
            points.push(Vec3::new(-0.05, 0.01, z));
            points.push(Vec3::new(0.05, 0.01, z));
            points.push(Vec3::new(0.0, top, z));
        }
        let caps = fit_shoe_upper(&points, 7);
        assert_eq!(caps.len(), 6);
        for c in &caps {
            assert!(
                c.bone_a == 7 && c.bone_b == 7,
                "shoe did not follow its bone"
            );
            assert!((c.radius - 0.05).abs() <= 1e-5, "shoe width = {}", c.radius);
        }
        assert!(
            caps[0].a.y + caps[0].radius > caps[5].a.y + caps[5].radius,
            "toe inflated to tongue height"
        );
        assert!(
            cuff_clearance(Vec3::new(0.0, 0.08, 0.02), &caps) > 0.0,
            "cuff remained pinned inside tongue"
        );
        assert_eq!(
            cuff_clearance(Vec3::new(0.0, 0.4, 0.02), &caps),
            0.0,
            "shoe changed shorts"
        );
        assert_eq!(
            cuff_clearance(Vec3::new(0.0, 0.08, 0.02), &[]),
            0.0,
            "bare feet changed cuff freedom"
        );
    }
}
