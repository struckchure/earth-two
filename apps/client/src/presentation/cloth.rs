//! Fitting cloth to a body: character/cloth.go. Hair and loose clothes move
//! with physics (verlet::Cloth): they swing and trail as the character
//! moves, hang under gravity, and are pushed out of the body. What sits
//! against the skin stays where the animation puts it; the further a part
//! is, along the garment, from anything touching the skin, the further it
//! can stray. Glasses and shoes are rigid.

use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

use super::ClothEnabled;
use super::footwear::{cuff_clearance, footwear_capsules};
use super::mesh::{ModelMeshes, ModelStore};
use super::outfit::{Garment, ModelPath, Slot};
use super::verlet::{Capsule, Cloth, ClothMeshSpec, closest_on_segment, cloth_freedom};

/// ClothKind is how one slot's items move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClothKind {
    /// Touching is how close to the skin a vertex is held.
    pub touching: f32,
    /// Rate is how much freedom a vertex gains per metre along the garment
    /// from the nearest held one, up to most metres.
    pub rate: f32,
    pub most: f32,
    /// Stiffness is the cloth's: how hard it's pulled back into shape.
    pub stiffness: f32,
    pub damping: f32,
    pub bending: f32,
    /// Support holds shoulder straps and necklines to their animated pose.
    /// It is the depth below the highest vertex, in metres; zero disables
    /// it.
    pub support: f32,
}

/// cloth_kind is how slot's items move, or None for the rigid ones.
pub fn cloth_kind(slot: Slot) -> Option<ClothKind> {
    Some(match slot {
        Slot::Hair => ClothKind {
            touching: 0.025,
            rate: 0.5,
            most: 0.15,
            stiffness: 0.035,
            damping: 0.08,
            bending: 0.35,
            support: 0.0,
        },
        Slot::Top | Slot::OnePiece => ClothKind {
            touching: 0.02,
            rate: 0.3,
            most: 0.08,
            stiffness: 0.045,
            damping: 0.12,
            bending: 0.6,
            support: 0.10,
        },
        Slot::Bottom => ClothKind {
            touching: 0.02,
            rate: 0.25,
            most: 0.05,
            stiffness: 0.06,
            damping: 0.16,
            bending: 0.75,
            support: 0.0,
        },
        // A coat hangs looser than what's under it, and swings further.
        Slot::Coat => ClothKind {
            touching: 0.03,
            rate: 0.3,
            most: 0.12,
            stiffness: 0.04,
            damping: 0.12,
            bending: 0.55,
            support: 0.10,
        },
        _ => return None,
    })
}

/// BodyCapsule describes one of the body's colliders: from one bone's
/// origin to another's, fitted to the vertices the listed bones move most.
/// The trunk, wider than it's deep, gets two side by side (split), one each
/// side. Each is cut into pieces along its length, each fitted to its own
/// stretch of the body: a limb narrows along its length, and one capsule
/// would be its widest part's size all the way.
struct BodyCapsule {
    from: &'static str,
    to: &'static str,
    bones: &'static [&'static str],
    split: bool,
    pieces: usize,
}

const BODY_CAPSULES: &[BodyCapsule] = &[
    BodyCapsule {
        from: "pelvis",
        to: "spine_02",
        bones: &["pelvis", "spine_01"],
        split: true,
        pieces: 3,
    },
    BodyCapsule {
        from: "spine_02",
        to: "neck_01",
        bones: &["spine_02", "spine_03"],
        split: true,
        pieces: 3,
    },
    BodyCapsule {
        from: "neck_01",
        to: "head",
        bones: &["neck_01"],
        split: false,
        pieces: 1,
    },
    BodyCapsule {
        from: "clavicle_l",
        to: "upperarm_l",
        bones: &["clavicle_l"],
        split: false,
        pieces: 1,
    },
    BodyCapsule {
        from: "clavicle_r",
        to: "upperarm_r",
        bones: &["clavicle_r"],
        split: false,
        pieces: 1,
    },
    BodyCapsule {
        from: "upperarm_l",
        to: "lowerarm_l",
        bones: &["upperarm_l"],
        split: false,
        pieces: 3,
    },
    BodyCapsule {
        from: "upperarm_r",
        to: "lowerarm_r",
        bones: &["upperarm_r"],
        split: false,
        pieces: 3,
    },
    BodyCapsule {
        from: "lowerarm_l",
        to: "hand_l",
        bones: &["lowerarm_l"],
        split: false,
        pieces: 3,
    },
    BodyCapsule {
        from: "lowerarm_r",
        to: "hand_r",
        bones: &["lowerarm_r"],
        split: false,
        pieces: 3,
    },
    BodyCapsule {
        from: "thigh_l",
        to: "calf_l",
        bones: &["thigh_l"],
        split: false,
        pieces: 4,
    },
    BodyCapsule {
        from: "thigh_r",
        to: "calf_r",
        bones: &["thigh_r"],
        split: false,
        pieces: 4,
    },
    BodyCapsule {
        from: "calf_l",
        to: "foot_l",
        bones: &["calf_l"],
        split: false,
        pieces: 3,
    },
    BodyCapsule {
        from: "calf_r",
        to: "foot_r",
        bones: &["calf_r"],
        split: false,
        pieces: 3,
    },
];

/// CAPSULE_FIT is the share of a capsule's vertices it reaches out to: just
/// under the skin, so cloth kept out of it (with its thickness) sits on the
/// body rather than off it.
pub const CAPSULE_FIT: f32 = 0.4;

/// Clothed marks a garment that has its Cloth, or needs none.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Clothed;

/// Rigid on a character's root keeps its clothes from moving with physics:
/// they move with its skeleton only. For a crowd: the cloth solver costs
/// each person a few milliseconds a frame.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Rigid;

/// BodyShape is a body model as clothe needs it: its colliders, and its
/// vertices in a grid, to find what's near the skin.
#[derive(Clone, Debug)]
pub struct BodyShape {
    pub capsules: Vec<Capsule>,
    grid: HashMap<[i32; 3], Vec<Vec3>>,
    cell: f32,
    surface: HashMap<[i32; 3], Vec<[Vec3; 3]>>,
}

impl Default for BodyShape {
    fn default() -> Self {
        BodyShape {
            capsules: vec![],
            grid: HashMap::new(),
            cell: 0.03,
            surface: HashMap::new(),
        }
    }
}

impl BodyShape {
    /// new reads body's bind pose: its vertices, and each one's bone.
    pub fn new(body: &ModelMeshes) -> BodyShape {
        let mut s = BodyShape::default();
        let mut by_bone: Vec<Vec<Vec3>> = vec![vec![]; body.skeleton.len()];
        for m in &body.meshes {
            if m.positions.is_empty() {
                continue;
            }
            let indices = m.triangles();
            let mut i = 0;
            while i + 2 < indices.len() {
                s.add_surface([
                    m.positions[indices[i] as usize],
                    m.positions[indices[i + 1] as usize],
                    m.positions[indices[i + 2] as usize],
                ]);
                i += 3;
            }
            for (v, p) in m.positions.iter().enumerate() {
                let k = s.key(*p);
                s.grid.entry(k).or_default().push(*p);
                if m.skinned() {
                    let bone = m.best_bone(v);
                    if bone < by_bone.len() {
                        by_bone[bone].push(*p);
                    }
                }
            }
        }
        s.capsules = fit_capsules(body, &by_bone);
        s
    }

    pub fn with_points(points: &[Vec3]) -> BodyShape {
        let mut s = BodyShape::default();
        for p in points {
            let k = s.key(*p);
            s.grid.entry(k).or_default().push(*p);
        }
        s
    }

    fn key(&self, p: Vec3) -> [i32; 3] {
        [
            (p.x / self.cell).floor() as i32,
            (p.y / self.cell).floor() as i32,
            (p.z / self.cell).floor() as i32,
        ]
    }

    /// near reports whether the body surface is within d of p. Testing
    /// faces also holds cloth over the centres of large body triangles,
    /// where no body vertex is nearby.
    pub fn near(&self, p: Vec3, d: f32) -> bool {
        let k = self.key(p);
        let r = (d / self.cell).ceil() as i32;
        for x in k[0] - r..=k[0] + r {
            for y in k[1] - r..=k[1] + r {
                for z in k[2] - r..=k[2] + r {
                    let cell = [x, y, z];
                    if let Some(faces) = self.surface.get(&cell)
                        && faces
                            .iter()
                            .any(|face| point_triangle_distance(p, face) < d)
                    {
                        return true;
                    }
                    if let Some(points) = self.grid.get(&cell)
                        && points.iter().any(|q| p.distance(*q) < d)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// add_surface indexes a face in the grid cells crossed by its bounding
    /// box.
    pub fn add_surface(&mut self, face: [Vec3; 3]) {
        let (mut lo, mut hi) = (self.key(face[0]), self.key(face[0]));
        for p in &face[1..] {
            let k = self.key(*p);
            for axis in 0..3 {
                lo[axis] = lo[axis].min(k[axis]);
                hi[axis] = hi[axis].max(k[axis]);
            }
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    self.surface.entry([x, y, z]).or_default().push(face);
                }
            }
        }
    }
}

pub fn point_triangle_distance(p: Vec3, face: &[Vec3; 3]) -> f32 {
    let (a, b, c) = (face[0], face[1], face[2]);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d00, d01, d11) = (ab.dot(ab), ab.dot(ac), ac.dot(ac));
    let (d20, d21) = (ap.dot(ab), ap.dot(ac));
    let denom = d00 * d11 - d01 * d01;
    if denom > 1e-12 {
        let (v, w) = (
            (d11 * d20 - d01 * d21) / denom,
            (d00 * d21 - d01 * d20) / denom,
        );
        if v >= 0.0 && w >= 0.0 && v + w <= 1.0 {
            return p.distance(a + ab * v + ac * w);
        }
    }
    p.distance(closest_on_segment(p, a, b))
        .min(p.distance(closest_on_segment(p, b, c)))
        .min(p.distance(closest_on_segment(p, c, a)))
}

/// fit_capsules fits BODY_CAPSULES, and a sphere for the head, to the
/// body's vertices by bone.
pub fn fit_capsules(body: &ModelMeshes, by_bone: &[Vec<Vec3>]) -> Vec<Capsule> {
    let sk = &body.skeleton;
    let mut out = vec![];
    for c in BODY_CAPSULES {
        let (Some(from), Some(to)) = (sk.index(c.from), sk.index(c.to)) else {
            continue;
        };
        let mut points = vec![];
        for name in c.bones {
            if let Some(i) = sk.index(name) {
                points.extend(&by_bone[i]);
            }
        }
        let groups = if c.split {
            split_sides(&points, sk.bind[from].translation.x)
        } else {
            vec![points]
        };
        for g in &groups {
            out.extend(fit_pieces(
                sk.bind[from].translation,
                sk.bind[to].translation,
                g,
                c.pieces,
                from as i32,
                to as i32,
            ));
        }
    }
    if let Some(head) = sk.index("head") {
        let pts = &by_bone[head];
        if !pts.is_empty() {
            let centre = pts.iter().sum::<Vec3>() / pts.len() as f32;
            // Upright: a head is taller than it is wide.
            let (a, b) = (
                centre + Vec3::new(0.0, -0.03, 0.0),
                centre + Vec3::new(0.0, 0.03, 0.0),
            );
            if let Some(mut k) = fit_capsule(a, b, pts) {
                k.bone_a = head as i32;
                k.bone_b = head as i32;
                out.push(k);
            }
        }
    }
    out
}

/// split_sides splits points into those left and right of x (the body
/// faces +Z in its bind pose, so X is across it).
pub fn split_sides(points: &[Vec3], x: f32) -> Vec<Vec<Vec3>> {
    let (mut left, mut right) = (vec![], vec![]);
    for p in points {
        if p.x < x {
            left.push(*p);
        } else {
            right.push(*p);
        }
    }
    vec![left, right]
}

/// fit_pieces cuts the segment a-b, carried by bones from and to, into n
/// capsules end to end, each fitted to the points alongside it. All but the
/// end at b move with from: they're along its bone.
pub fn fit_pieces(a: Vec3, b: Vec3, points: &[Vec3], n: usize, from: i32, to: i32) -> Vec<Capsule> {
    let n = n.max(1);
    let ab = b - a;
    let l = ab.dot(ab);
    let mut pieces: Vec<Vec<Vec3>> = vec![vec![]; n];
    for p in points {
        let mut i = 0;
        if l > 1e-12 {
            let t = (*p - a).dot(ab) / l;
            i = ((t * n as f32) as i32).clamp(0, n as i32 - 1) as usize;
        }
        pieces[i].push(*p);
    }
    let mut out = vec![];
    for (i, pts) in pieces.iter().enumerate() {
        let start = a.lerp(b, i as f32 / n as f32);
        let end = a.lerp(b, (i + 1) as f32 / n as f32);
        let Some(mut k) = fit_capsule(start, end, pts) else {
            continue;
        };
        k.bone_a = from;
        k.bone_b = from;
        if i == n - 1 {
            k.bone_b = to;
        }
        out.push(k);
    }
    out
}

/// fit_capsule centres the segment a-b among points (bones run nearer the
/// back than the middle) and gives it the radius reaching CAPSULE_FIT of
/// them.
pub fn fit_capsule(a: Vec3, b: Vec3, points: &[Vec3]) -> Option<Capsule> {
    if points.is_empty() {
        return None;
    }
    let mut off = Vec3::ZERO;
    for p in points {
        off += *p - closest_on_segment(*p, a, b);
    }
    off /= points.len() as f32;
    let (a, b) = (a + off, b + off);
    let mut d: Vec<f32> = points
        .iter()
        .map(|p| p.distance(closest_on_segment(*p, a, b)))
        .collect();
    d.sort_by(f32::total_cmp);
    Some(Capsule {
        a,
        b,
        radius: d[(CAPSULE_FIT * (d.len() - 1) as f32) as usize],
        bone_a: 0,
        bone_b: 0,
    })
}

/// cloth_pins says which of a garment's vertices are held. The shoulders
/// support a top even when its straps stand further off the skin than
/// touching. Letting those supports simulate lifts the neckline and makes
/// the rest of the garment hang from moving, floating anchors.
pub fn cloth_pins(vertices: &[Vec3], body: &BodyShape, kind: &ClothKind) -> Vec<bool> {
    let mut pinned = vec![false; vertices.len()];
    if vertices.is_empty() {
        return pinned;
    }
    let top = vertices.iter().map(|p| p.y).fold(vertices[0].y, f32::max);
    for (v, p) in vertices.iter().enumerate() {
        pinned[v] = kind.support > 0.0 && p.y >= top - kind.support || body.near(*p, kind.touching);
    }
    pinned
}

/// fit_cloth works out how each of garment's meshes moves on body; meshes
/// that don't move (and the skin patches, skin) are left out.
pub fn fit_cloth(
    garment: &ModelMeshes,
    body: &BodyShape,
    kind: &ClothKind,
    skin: &[usize],
    footwear: &[Capsule],
) -> std::collections::BTreeMap<usize, ClothMeshSpec> {
    let mut out = std::collections::BTreeMap::new();
    for (i, m) in garment.meshes.iter().enumerate() {
        if skin.contains(&i) || !m.skinned() {
            continue;
        }
        let mut pinned = cloth_pins(&m.positions, body, kind);
        for (v, p) in m.positions.iter().enumerate() {
            if cuff_clearance(*p, footwear) > 0.0 {
                pinned[v] = false;
            }
        }
        let mut freedom =
            cloth_freedom(&m.positions, &m.triangles(), &pinned, kind.rate, kind.most);
        for (v, p) in m.positions.iter().enumerate() {
            freedom[v] = freedom[v].max(cuff_clearance(*p, footwear));
        }
        if freedom.iter().any(|f| *f > 0.0) {
            out.insert(i, ClothMeshSpec { freedom });
        }
    }
    out
}

/// ClothCache keeps what clothe works out per model.
#[derive(Resource, Default)]
pub struct ClothCache {
    fits: HashMap<ClothKey, std::collections::BTreeMap<usize, ClothMeshSpec>>,
    bodies: HashMap<String, Arc<BodyShape>>,
    shoes: HashMap<String, Arc<Vec<Capsule>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ClothKey {
    garment: String,
    body: String,
    footwear: Option<String>,
    slot: Slot,
}

/// clothe gives each new garment a Cloth fitted to the body it's on, but on
/// a Rigid character (or with the solver off: then nothing moves but the
/// skeleton).
#[allow(clippy::type_complexity)]
pub fn clothe(
    mut commands: Commands,
    garments: Query<(Entity, &Garment, &ModelPath, &ChildOf), Without<Clothed>>,
    models: Query<(&ModelPath, &ChildOf)>,
    rigid: Query<(), With<Rigid>>,
    store: Res<ModelStore>,
    enabled: Res<ClothEnabled>,
    mut cache: Local<ClothCache>,
) {
    if !enabled.0 {
        return;
    }
    for (e, g, m, parent) in &garments {
        let Ok((pm, body_parent)) = models.get(parent.parent()) else {
            continue;
        };
        let (Some(garment), Some(body)) = (store.get(&m.0), store.get(&pm.0)) else {
            continue; // not loaded yet
        };
        let kind = cloth_kind(g.slot);
        let still = rigid.contains(body_parent.parent());
        let Some(kind) = kind.filter(|_| !still) else {
            commands.entity(e).insert(Clothed);
            continue;
        };
        let shape = cache
            .bodies
            .entry(pm.0.clone())
            .or_insert_with(|| Arc::new(BodyShape::new(&body)))
            .clone();
        let mut shoe_caps: Arc<Vec<Capsule>> = Arc::new(vec![]);
        if let Some(footwear) = &g.footwear {
            let Some(shoe) = store.get(footwear) else {
                continue;
            };
            shoe_caps = cache
                .shoes
                .entry(footwear.clone())
                .or_insert_with(|| Arc::new(footwear_capsules(&shoe)))
                .clone();
        }
        let key = ClothKey {
            garment: m.0.clone(),
            body: pm.0.clone(),
            footwear: g.footwear.clone(),
            slot: g.slot,
        };
        let fit = cache
            .fits
            .entry(key)
            .or_insert_with(|| fit_cloth(&garment, &shape, &kind, &g.skin, &shoe_caps))
            .clone();
        if fit.is_empty() {
            commands.entity(e).insert(Clothed);
            continue;
        }
        let mut colliders = shape.capsules.clone();
        colliders.extend(shoe_caps.iter());
        commands.entity(e).insert((
            Clothed,
            Cloth {
                meshes: fit,
                colliders,
                stiffness: kind.stiffness,
                damping: kind.damping,
                bending: kind.bending,
                ..Default::default()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ring is n points round the Y axis at radius r and height y, centred
    /// on c.
    fn ring(c: Vec3, r: f32, y: f32, n: usize) -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                c + Vec3::new(r * a.cos(), y, r * a.sin())
            })
            .collect()
    }

    // character/cloth_test.go TestFitCapsuleCentresOnTheBody.
    #[test]
    fn fit_capsule_centres_on_the_body() {
        let mut pts = vec![];
        for y in [0.1, 0.3, 0.5] {
            pts.extend(ring(Vec3::new(0.0, 0.0, 0.1), 0.06, y, 32));
        }
        let k = fit_capsule(Vec3::new(0.0, 0.0, 0.05), Vec3::new(0.0, 0.6, 0.05), &pts).unwrap();
        assert!(
            (k.a.z - 0.1).abs() <= 1e-3 && (k.b.z - 0.1).abs() <= 1e-3,
            "{k:?}"
        );
        assert!((k.radius - 0.06).abs() <= 1e-3, "radius {}", k.radius);
        assert!(fit_capsule(Vec3::ZERO, Vec3::Y, &[]).is_none());
    }

    // TestBodyShapeNear.
    #[test]
    fn body_shape_near() {
        let s = BodyShape::with_points(&[Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0)]);
        for (p, want) in [
            (Vec3::new(0.01, 0.0, 0.0), true),
            (Vec3::new(-0.015, 0.01, 0.0), true),
            (Vec3::new(0.03, 0.0, 0.0), false),
            (Vec3::new(0.49, 0.0, 0.005), true),
            (Vec3::new(0.25, 0.0, 0.0), false),
        ] {
            assert_eq!(s.near(p, 0.025), want, "near({p})");
        }
    }

    // TestSplitSides.
    #[test]
    fn split_sides_by_x() {
        let got = split_sides(
            &[
                Vec3::new(-0.1, 0.0, 0.0),
                Vec3::new(0.2, 0.0, 0.0),
                Vec3::new(0.05, 0.0, 0.0),
                Vec3::new(-0.3, 0.0, 0.0),
            ],
            0.02,
        );
        assert!(got.len() == 2 && got[0].len() == 2 && got[1].len() == 2);
        assert!(got[0][1].x == -0.3 && got[1][1].x == 0.05);
    }

    // TestFitPiecesFollowsTheTaper.
    #[test]
    fn fit_pieces_follows_the_taper() {
        let mut pts = vec![];
        for i in 0..9 {
            let y = -0.05 * i as f32;
            pts.extend(ring(Vec3::ZERO, 0.08 - 0.03 * (-y / 0.4), y, 32));
        }
        let (a, b) = (Vec3::ZERO, Vec3::new(0.0, -0.4, 0.0));
        let got = fit_pieces(a, b, &pts, 4, 1, 2);
        assert_eq!(got.len(), 4);
        let whole = fit_capsule(a, b, &pts).unwrap();
        let knee = got[3].radius;
        assert!(
            knee < whole.radius && knee <= 0.056,
            "knee {knee} whole {}",
            whole.radius
        );
        assert!(got[0].radius >= 0.07, "hip {}", got[0].radius);
        for (i, k) in got.iter().enumerate() {
            let want_b = if i == 3 { 2 } else { 1 };
            assert!(
                k.bone_a == 1 && k.bone_b == want_b,
                "piece {i} carried by {}-{}",
                k.bone_a,
                k.bone_b
            );
        }
        assert!(got[0].a.y.abs() <= 1e-4 && (got[3].b.y + 0.4).abs() <= 1e-4);
    }

    // TestBodyShapeNearTriangleInterior.
    #[test]
    fn body_shape_near_triangle_interior() {
        let mut s = BodyShape::default();
        s.add_surface([
            Vec3::new(-0.12, 0.0, 0.0),
            Vec3::new(0.12, 0.0, 0.0),
            Vec3::new(0.0, 0.18, 0.0),
        ]);
        for (p, want) in [
            (Vec3::new(0.0, 0.06, 0.01), true),
            (Vec3::new(0.0, 0.06, -0.01), true),
            (Vec3::new(0.0, 0.06, 0.03), false),
            (Vec3::new(0.2, 0.06, 0.0), false),
            (Vec3::new(0.0, -0.01, 0.0), true),
        ] {
            assert_eq!(s.near(p, 0.02), want, "near({p})");
        }
    }

    // TestPointTriangleDistanceDegenerate.
    #[test]
    fn point_triangle_distance_degenerate() {
        let face = [
            Vec3::ZERO,
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(0.2, 0.0, 0.0),
        ];
        let got = point_triangle_distance(Vec3::new(0.05, 0.01, 0.0), &face);
        assert!((got - 0.01).abs() <= 1e-5, "{got}");
    }

    // TestClothPinsShoulderSupports.
    #[test]
    fn cloth_pins_shoulder_supports() {
        let body = BodyShape::default(); // all test points stand away from skin
        let vertices = [
            Vec3::new(0.0, 1.5, 0.0),
            Vec3::new(0.2, 1.46, 0.0),
            Vec3::new(0.0, 1.39, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let got = cloth_pins(&vertices, &body, &cloth_kind(Slot::Top).unwrap());
        assert_eq!(got, vec![true, true, false, false]);
        let pins = cloth_pins(&vertices, &body, &cloth_kind(Slot::Bottom).unwrap());
        assert!(
            !pins.contains(&true),
            "shoulder rule applied to trousers: {pins:?}"
        );
    }
}
