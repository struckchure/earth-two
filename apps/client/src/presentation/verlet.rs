//! The cloth solver: illusion's render/cloth.go, with the GPU left out.
//! Hair and loose clothes move with physics: each simulated vertex follows
//! where the animation puts it, but it has weight: it lags behind when the
//! entity moves, swings under gravity, keeps its distance to its neighbours,
//! resists folding, and is pushed out of the colliders. The model is taken
//! to be the shape the cloth hangs in at rest. Freedom limits how far it can
//! stray, and it never goes in behind where the animation puts it.

use bevy::math::{Mat4, Quat, Vec3};
use bevy::prelude::Component;
use std::collections::{BTreeMap, HashMap};

use super::mesh::ModelMeshes;

/// Cloth lets parts of an entity's skinned model move with physics. Its
/// state lives in the component: a fresh Cloth starts where the pose is.
#[derive(Component, Clone, Debug, Default)]
pub struct Cloth {
    /// The simulated meshes, by index in the model. The rest are only
    /// skinned.
    pub meshes: BTreeMap<usize, ClothMeshSpec>,
    /// Colliders push the cloth out; they move with the model's bones.
    pub colliders: Vec<Capsule>,
    /// Gravity is the acceleration on the cloth, in world space. Zero means
    /// 9.8 m/s² down.
    pub gravity: Vec3,
    /// Damping is the share of its speed the cloth loses each 60th of a
    /// second (0 to 1). Zero means 0.03.
    pub damping: f32,
    /// Stiffness is the share of the way back to its animated shape the
    /// cloth is pulled each 60th of a second (0 to 1). Zero means 0.02.
    pub stiffness: f32,
    /// Bending is how hard the cloth keeps its folds, each pass (0 to 1).
    /// Zero means 0.5.
    pub bending: f32,
    /// Thickness is how far the cloth keeps off the colliders, in metres.
    /// Zero means 0.005.
    pub thickness: f32,

    pub(crate) state: Option<ClothState>,
}

/// ClothMeshSpec says how one mesh moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothMeshSpec {
    /// Freedom is, for each vertex of the mesh (in the mesh's order), how
    /// far in metres it can move from where the animation puts it; 0 pins it
    /// there. Vertices at the same place (split for texture seams) move as
    /// one, with the most freedom among them.
    pub freedom: Vec<f32>,
}

/// Capsule is a collider: a segment and the radius around it. Its ends a
/// and b are points in the model's bind pose, carried by bones bone_a and
/// bone_b (by index in its skeleton) as they move. Equal ends make a sphere.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub bone_a: i32,
    pub bone_b: i32,
    pub radius: f32,
}

/// The simulation's time step, in seconds.
pub const CLOTH_STEP: f32 = 1.0 / 60.0;
/// Steps per frame at most; a slower frame runs slow. Few, so that a machine
/// a step is slow on isn't slowed further by owing more of them.
pub const CLOTH_MAX_STEPS: u32 = 2;
/// Constraint passes per step.
pub const CLOTH_ITERATIONS: u32 = 6;
/// Metres: moving further in a frame restarts the cloth.
pub const CLOTH_TELEPORT: f32 = 1.0;

/// ClothState is the running simulation.
#[derive(Clone, Debug, Default)]
pub struct ClothState {
    pub(crate) meshes: BTreeMap<usize, ClothMesh>,
    /// Every skinned mesh as posed this frame.
    pub posed: Vec<PosedMesh>,
    /// The model's origin last frame, in world space.
    origin: Vec3,
    /// Simulated time owed, in seconds.
    spare: f32,
}

/// PosedMesh is a mesh's vertices as posed this frame, in model space.
#[derive(Clone, Debug, Default)]
pub struct PosedMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

/// ClothMesh is one mesh's simulation. Vertices at the same bind position
/// share a particle.
#[derive(Clone, Debug, Default)]
pub struct ClothMesh {
    /// vertex -> particle
    pub(crate) particle: Vec<i32>,
    /// particle -> one of its vertices
    pub(crate) first: Vec<i32>,
    /// particle -> how far it can stray
    pub(crate) freedom: Vec<f32>,
    pub(crate) edges: Vec<[i32; 2]>,
    /// The far corners of each pair of triangles sharing an edge.
    pub(crate) bends: Vec<[i32; 2]>,
    /// Rest lengths for the current physics pose.
    edge_length: Vec<f32>,
    bend_length: Vec<f32>,
    step_target: Vec<Vec3>,
    draw_position: Vec<Vec3>,
    rotations: Vec<Quat>,
    /// The particles that move.
    pub(crate) free: Vec<i32>,

    /// Displacement from the pose at the last two physics steps.
    draw_prev: Vec<Vec3>,
    draw_offset: Vec<Vec3>,
    /// Triangle corners as welded particle indices.
    triangles: Vec<i32>,
    /// Area-weighted normals before and after deformation.
    pose_normal: Vec<Vec3>,
    draw_normal: Vec<Vec3>,
    /// Particles, world space.
    pub(crate) pos: Vec<Vec3>,
    pub(crate) prev: Vec<Vec3>,
    /// Where the pose puts each particle, world space.
    pub(crate) target: Vec<Vec3>,
    pub(crate) last_target: Vec<Vec3>,
    /// The posed surface's normal at each moving particle, world space.
    pub(crate) normal: Vec<Vec3>,
    /// The colliders each moving particle can reach this frame (see reach):
    /// those of free[i] are near[near_end[i-1]..near_end[i]], by index.
    pub(crate) near: Vec<i32>,
    pub(crate) near_end: Vec<i32>,
    /// How far from its pose each of free can get this frame.
    within: Vec<f32>,
    /// The colliders near the mesh at all.
    close: Vec<CloseCapsule>,
    pub(crate) started: bool,
}

fn key(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

impl ClothMesh {
    /// new builds the simulation of a mesh with vertices (bind positions)
    /// and triangles (vertex indices, three a triangle).
    pub fn new(vertices: &[Vec3], triangles: &[i32], freedom: &[f32]) -> ClothMesh {
        let mut c = ClothMesh {
            particle: vec![0; vertices.len()],
            ..Default::default()
        };
        let mut at: HashMap<[u32; 3], i32> = HashMap::new();
        for (i, v) in vertices.iter().enumerate() {
            let p = *at.entry(key(*v)).or_insert_with(|| {
                c.first.push(i as i32);
                c.freedom.push(0.0);
                c.first.len() as i32 - 1
            });
            c.particle[i] = p;
            if i < freedom.len() && freedom[i] > c.freedom[p as usize] {
                c.freedom[p as usize] = freedom[i];
            }
        }
        let mut seen: HashMap<[i32; 2], bool> = HashMap::new();
        // edge -> the far corner of the first triangle on it
        let mut far: HashMap<[i32; 2], i32> = HashMap::new();
        let mut bent: HashMap<[i32; 2], bool> = HashMap::new();
        let mut t = 0;
        while t + 2 < triangles.len() {
            for k in 0..3 {
                let (mut a, mut b) = (
                    c.particle[triangles[t + k] as usize],
                    c.particle[triangles[t + (k + 1) % 3] as usize],
                );
                let o = c.particle[triangles[t + (k + 2) % 3] as usize];
                if a == b {
                    continue;
                }
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                let e = [a, b];
                match far.get(&e) {
                    None => {
                        far.insert(e, o);
                    }
                    Some(&other) => {
                        if other != o
                            && (c.freedom[other as usize] > 0.0 || c.freedom[o as usize] > 0.0)
                        {
                            let pair = [other.min(o), other.max(o)];
                            if let std::collections::hash_map::Entry::Vacant(e) = bent.entry(pair) {
                                e.insert(true);
                                c.bends.push(pair);
                            }
                        }
                    }
                }
                if (c.freedom[a as usize] == 0.0 && c.freedom[b as usize] == 0.0)
                    || seen.contains_key(&e)
                {
                    continue;
                }
                seen.insert(e, true);
                c.edges.push(e);
            }
            t += 3;
        }
        for (p, f) in c.freedom.iter().enumerate() {
            if *f > 0.0 {
                c.free.push(p as i32);
            }
        }
        for v in triangles {
            c.triangles.push(c.particle[*v as usize]);
        }
        let n = c.first.len();
        c.edge_length = vec![0.0; c.edges.len()];
        c.bend_length = vec![0.0; c.bends.len()];
        c.step_target = vec![Vec3::ZERO; n];
        c.draw_position = vec![Vec3::ZERO; n];
        c.rotations = vec![Quat::IDENTITY; n];
        c.draw_prev = vec![Vec3::ZERO; n];
        c.draw_offset = vec![Vec3::ZERO; n];
        c.pose_normal = vec![Vec3::ZERO; n];
        c.draw_normal = vec![Vec3::ZERO; n];
        c.pos = vec![Vec3::ZERO; n];
        c.prev = vec![Vec3::ZERO; n];
        c.target = vec![Vec3::ZERO; n];
        c.last_target = vec![Vec3::ZERO; n];
        c.normal = vec![Vec3::ZERO; n];
        c
    }

    pub fn particle_count(&self) -> usize {
        self.first.len()
    }

    /// step advances the simulation by h seconds, the targets moving from
    /// last_target to target over the step's share t of the frame.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &mut self,
        h: f32,
        t: f32,
        gravity: Vec3,
        keep: f32,
        stiffness: f32,
        bending: f32,
        colliders: &[WorldCapsule],
    ) {
        for p in 0..self.first.len() {
            self.step_target[p] = self.target_at(p as i32, t);
        }
        for (i, e) in self.edges.iter().enumerate() {
            self.edge_length[i] =
                self.step_target[e[0] as usize].distance(self.step_target[e[1] as usize]);
        }
        for (i, e) in self.bends.iter().enumerate() {
            self.bend_length[i] =
                self.step_target[e[0] as usize].distance(self.step_target[e[1] as usize]);
        }
        let g = gravity * (h * h);
        // The pull back to shape aims above the pose by as much as gravity
        // would drag the cloth below it at rest: the pose is how it hangs.
        let hold = g * (-(1.0 - stiffness) / stiffness);
        for &p in &self.free {
            let p = p as usize;
            let target = self.step_target[p];
            let mut x = self.pos[p];
            let v = (x - self.prev[p]) * keep;
            self.prev[p] = x;
            x = x + v + g;
            self.pos[p] = x.lerp(target + hold, stiffness);
        }
        for _ in 0..CLOTH_ITERATIONS {
            self.keep_all_apart(true, 1.0);
            self.keep_all_apart(false, bending);
            for i in 0..self.free.len() {
                let p = self.free[i] as usize;
                let target = self.step_target[p];
                // Within its freedom of where the pose puts it, and not
                // behind it.
                let mut off = self.pos[p] - target;
                if off.length() > self.freedom[p] {
                    off = off.normalize() * self.freedom[p];
                }
                let inward = off.dot(self.normal[p]);
                if inward < 0.0 {
                    off -= self.normal[p] * inward;
                }
                // And out of the body, whatever the pose says: the pose
                // sinks loose cloth into a leg that swings into it.
                self.pos[p] = self.collide(i, target + off, target, colliders);
            }
        }
    }

    /// reach works out which colliders each moving particle can touch this
    /// frame. A particle stays within its freedom of where the pose puts
    /// it, and that moves from last_target to target, so only a collider
    /// within that (and CLOTH_PUSH more: one collider can push it into
    /// another) matters. Most particles are nowhere near most limbs: this
    /// leaves each a few to test, where every constraint pass of every step
    /// would otherwise test them all.
    pub(crate) fn reach(&mut self, colliders: &[WorldCapsule]) {
        self.near.clear();
        if self.near_end.len() != self.free.len() {
            self.near_end = vec![0; self.free.len()];
            self.within = vec![0.0; self.free.len()];
        }
        if self.free.is_empty() {
            return;
        }
        // How far each particle can get from where the pose puts it, and
        // the box they're all in: the colliders nowhere near it are left
        // out first.
        let (mut lo, mut hi) = (
            self.target[self.free[0] as usize],
            self.target[self.free[0] as usize],
        );
        let mut furthest = 0.0f32;
        for i in 0..self.free.len() {
            let p = self.free[i] as usize;
            let x = self.target[p];
            let within = self.freedom[p] + CLOTH_PUSH + self.last_target[p].distance(x);
            self.within[i] = within;
            furthest = furthest.max(within);
            lo = lo.min(x);
            hi = hi.max(x);
        }
        self.close.clear();
        for (j, k) in colliders.iter().enumerate() {
            let r = k.radius + furthest;
            let (klo, khi) = (k.a.min(k.b), k.a.max(k.b));
            if hi.x < klo.x - r
                || lo.x > khi.x + r
                || hi.y < klo.y - r
                || lo.y > khi.y + r
                || hi.z < klo.z - r
                || lo.z > khi.z + r
            {
                continue;
            }
            self.close.push(CloseCapsule {
                index: j as i32,
                a: k.a,
                b: k.b,
                radius: k.radius,
                lo: klo - Vec3::splat(k.radius),
                hi: khi + Vec3::splat(k.radius),
            });
        }
        for i in 0..self.free.len() {
            let p = self.free[i] as usize;
            let (x, within) = (self.target[p], self.within[i]);
            for k in &self.close {
                if x.x < k.lo.x - within
                    || x.x > k.hi.x + within
                    || x.y < k.lo.y - within
                    || x.y > k.hi.y + within
                    || x.z < k.lo.z - within
                    || x.z > k.hi.z + within
                {
                    continue;
                }
                let r = k.radius + within;
                if x.distance_squared(closest_on_segment(x, k.a, k.b)) >= r * r {
                    continue;
                }
                self.near.push(k.index);
            }
            self.near_end[i] = self.near.len() as i32;
        }
    }

    /// collide pushes x, where free[i] is, out of the colliders it can reach
    /// (or of them all, if reach hasn't said which those are).
    pub(crate) fn collide(
        &self,
        i: usize,
        mut x: Vec3,
        target: Vec3,
        colliders: &[WorldCapsule],
    ) -> Vec3 {
        if self.near_end.len() != self.free.len() {
            return collide(x, target, colliders);
        }
        let start = if i > 0 { self.near_end[i - 1] } else { 0 };
        for &j in &self.near[start as usize..self.near_end[i] as usize] {
            x = push_out(x, target, &colliders[j as usize]);
        }
        x
    }

    /// keep_all_apart moves each of the pairs (the edges, or the bends) the
    /// share k of the way back to the distance the pose puts between them;
    /// a pinned particle stays where the pose has it.
    fn keep_all_apart(&mut self, edges: bool, k: f32) {
        let (pairs, rest) = if edges {
            (&self.edges, &self.edge_length)
        } else {
            (&self.bends, &self.bend_length)
        };
        let (pos, freedom, pose) = (&mut self.pos, &self.freedom, &self.step_target);
        for (i, e) in pairs.iter().enumerate() {
            let (a, b) = (e[0] as usize, e[1] as usize);
            let (move_a, move_b) = (freedom[a] > 0.0, freedom[b] > 0.0);
            if !move_a && !move_b {
                continue;
            }
            let mut pa = pos[a];
            let mut pb = pos[b];
            if !move_a {
                pa = pose[a];
            }
            if !move_b {
                pb = pose[b];
            }
            let d = pb - pa;
            let length = d.length();
            if length < 1e-6 {
                continue;
            }
            let mut fix = k * (length - rest[i]) / length;
            if move_a && move_b {
                fix /= 2.0;
                pos[a] = pa + d * fix;
                pos[b] = pb - d * fix;
            } else if move_a {
                pos[a] = pa + d * fix;
            } else {
                pos[b] = pb - d * fix;
            }
        }
    }

    /// at is where particle p is: pinned ones are wherever the pose puts
    /// them.
    #[cfg(test)]
    pub(crate) fn at(&self, p: i32, t: f32) -> Vec3 {
        if self.freedom[p as usize] > 0.0 {
            return self.pos[p as usize];
        }
        self.target_at(p, t)
    }

    fn target_at(&self, p: i32, t: f32) -> Vec3 {
        self.last_target[p as usize].lerp(self.target[p as usize], t)
    }

    /// draw_near is where free[i] is drawn, against the colliders it can
    /// reach.
    fn draw_near(&self, i: usize, alpha: f32, colliders: &[WorldCapsule]) -> Vec3 {
        let p = self.free[i];
        self.collide(
            i,
            self.target[p as usize] + self.drawn_offset(p, alpha),
            self.target[p as usize],
            colliders,
        )
    }

    /// drawn_offset is how far from its pose particle p is drawn, alpha of
    /// the way from the last physics step to this one.
    fn drawn_offset(&self, p: i32, alpha: f32) -> Vec3 {
        let p = p as usize;
        let mut off = self.draw_prev[p].lerp(self.draw_offset[p], alpha);
        let length = off.length();
        if length > self.freedom[p] {
            off *= self.freedom[p] / length;
        }
        let inward = off.dot(self.normal[p]);
        if inward < 0.0 {
            off -= self.normal[p] * inward;
        }
        off
    }

    /// surface_normals accumulates triangle areas across welded UV seams.
    fn surface_normals(&self, out: &mut [Vec3], positions: &[Vec3]) {
        out.fill(Vec3::ZERO);
        let mut i = 0;
        while i + 2 < self.triangles.len() {
            let (a, b, d) = (
                self.triangles[i] as usize,
                self.triangles[i + 1] as usize,
                self.triangles[i + 2] as usize,
            );
            let (pa, pb, pd) = (
                positions[self.first[a] as usize],
                positions[self.first[b] as usize],
                positions[self.first[d] as usize],
            );
            let n = (pb - pa).cross(pd - pa);
            out[a] += n;
            out[b] += n;
            out[d] += n;
            i += 3;
        }
    }

    /// deform_normals rotates the authored shading normals by the cloth's
    /// surface deformation; this preserves their smoothing and detail
    /// instead of flattening the mesh.
    fn deform_normals(&mut self, positions: &[Vec3], normals: &mut [Vec3]) {
        let mut draw_normal = std::mem::take(&mut self.draw_normal);
        self.surface_normals(&mut draw_normal, positions);
        self.draw_normal = draw_normal;
        for &p in &self.free {
            let p = p as usize;
            self.rotations[p] = Quat::IDENTITY;
            if self.pose_normal[p].length_squared() >= 1e-12
                && self.draw_normal[p].length_squared() >= 1e-12
            {
                self.rotations[p] = Quat::from_rotation_arc(
                    self.pose_normal[p].normalize(),
                    self.draw_normal[p].normalize(),
                );
            }
        }
        for (v, &p) in self.particle.iter().enumerate() {
            if self.freedom[p as usize] > 0.0 {
                normals[v] = (self.rotations[p as usize] * normals[v]).normalize_or_zero();
            }
        }
    }
}

/// CLOTH_PUSH is how far past its freedom a particle is taken to be pushed
/// by one collider into the reach of another (see reach).
pub const CLOTH_PUSH: f32 = 0.03;

/// CloseCapsule is a collider near a mesh, with its bounds.
#[derive(Clone, Copy, Debug, Default)]
struct CloseCapsule {
    index: i32,
    a: Vec3,
    b: Vec3,
    radius: f32,
    lo: Vec3,
    hi: Vec3,
}

/// WorldCapsule is a Capsule placed in world space for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldCapsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

/// collide pushes x out of the capsules. A point right on a capsule's axis
/// goes out towards target, where the pose puts it.
pub fn collide(mut x: Vec3, target: Vec3, colliders: &[WorldCapsule]) -> Vec3 {
    for k in colliders {
        x = push_out(x, target, k);
    }
    x
}

/// push_out pushes x out of capsule k.
pub fn push_out(x: Vec3, target: Vec3, k: &WorldCapsule) -> Vec3 {
    // Reject by the capsule's bounds before doing segment projection and
    // square roots.
    let r = k.radius;
    let (lo, hi) = (k.a.min(k.b), k.a.max(k.b));
    if x.x < lo.x - r
        || x.x > hi.x + r
        || x.y < lo.y - r
        || x.y > hi.y + r
        || x.z < lo.z - r
        || x.z > hi.z + r
    {
        return x;
    }
    let closest = closest_on_segment(x, k.a, k.b);
    let mut d = x - closest;
    let dist_sq = d.length_squared();
    if dist_sq >= r * r {
        return x;
    }
    let mut dist = dist_sq.sqrt();
    if dist < 1e-6 {
        d = target - closest;
        dist = target.distance(closest);
        if dist < 1e-6 {
            return x;
        }
    }
    closest + d * (r / dist)
}

pub fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let l = ab.dot(ab);
    if l < 1e-12 {
        return a;
    }
    let t = (p - a).dot(ab) / l;
    a + ab * t.clamp(0.0, 1.0)
}

/// skin poses the mesh's bind vertices like raylib's CPU skinning does.
pub fn skin(
    out: &mut [Vec3],
    vertices: &[Vec3],
    joints: &[[u16; 4]],
    weights: &[[f32; 4]],
    bones: &[Mat4],
) {
    for (i, v) in vertices.iter().enumerate() {
        let mut s = Vec3::ZERO;
        for j in 0..4 {
            let w = weights[i][j];
            if w == 0.0 {
                continue;
            }
            s += bones[joints[i][j] as usize].transform_point3(*v) * w;
        }
        out[i] = s;
    }
}

/// skin_normal poses normal n of a vertex with its four bones and weights,
/// turning it with the bones.
pub fn skin_normal(n: Vec3, joints: &[u16; 4], weights: &[f32; 4], bones: &[Mat4]) -> Vec3 {
    let mut s = Vec3::ZERO;
    for j in 0..4 {
        let w = weights[j];
        if w != 0.0 {
            s += bones[joints[j] as usize].transform_vector3(n) * w;
        }
    }
    s
}

/// place_colliders poses the capsules with their bones (bones being the
/// model's bone matrices, which take the bind pose to the current one), puts
/// them in world space and fattens them by the cloth's thickness.
pub fn place_colliders(
    capsules: &[Capsule],
    bones: &[Mat4],
    matrix: Mat4,
    thickness: f32,
) -> Vec<WorldCapsule> {
    let scale = matrix.x_axis.truncate().length();
    let mut out = Vec::with_capacity(capsules.len());
    for k in capsules {
        if k.bone_a < 0
            || k.bone_b < 0
            || k.bone_a as usize >= bones.len()
            || k.bone_b as usize >= bones.len()
        {
            continue;
        }
        out.push(WorldCapsule {
            a: matrix.transform_point3(bones[k.bone_a as usize].transform_point3(k.a)),
            b: matrix.transform_point3(bones[k.bone_b as usize].transform_point3(k.b)),
            radius: k.radius * scale + thickness,
        });
    }
    out
}

/// ClothFrame samples the animated targets at the actual fixed-step times.
/// alpha blends the two most recent displacements for presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClothFrame {
    pub steps: u32,
    pub first: f32,
    pub stride: f32,
    pub alpha: f32,
}

impl ClothState {
    /// schedule owes the frame's time to the fixed-step simulation.
    pub fn schedule(&mut self, dt: f32) -> ClothFrame {
        let dt = dt.max(0.0);
        self.spare += dt;
        let mut steps = (self.spare / CLOTH_STEP) as u32;
        self.spare -= steps as f32 * CLOTH_STEP;
        steps = steps.min(CLOTH_MAX_STEPS);
        let mut f = ClothFrame {
            steps,
            alpha: self.spare / CLOTH_STEP,
            ..Default::default()
        };
        if dt > 0.0 && steps > 0 {
            f.stride = CLOTH_STEP / dt;
            f.first = 1.0 - (self.spare + (steps - 1) as f32 * CLOTH_STEP) / dt;
        }
        f
    }
}

/// simulate poses model with bones (the matrices that take the bind pose to
/// the current one) and advances cloth on it, drawn with matrix, by dt
/// seconds. It skins every mesh itself. Afterwards cloth.state's posed
/// meshes hold each skinned mesh's positions and normals in model space,
/// the simulated ones moved by the cloth.
pub fn simulate(cloth: &mut Cloth, model: &ModelMeshes, bones: &[Mat4], matrix: Mat4, dt: f32) {
    let meshes = &model.meshes;
    if cloth.state.is_none() {
        let mut state = ClothState::default();
        for (&i, cm) in &cloth.meshes {
            let Some(m) = meshes.get(i) else { continue };
            if !m.skinned() {
                continue;
            }
            state
                .meshes
                .insert(i, ClothMesh::new(&m.positions, &m.triangles(), &cm.freedom));
        }
        cloth.state = Some(state);
    }
    let s = cloth.state.as_mut().unwrap();
    let origin = matrix.transform_point3(Vec3::ZERO);
    let restart = origin.distance(s.origin) > CLOTH_TELEPORT;
    s.origin = origin;

    let gravity = if cloth.gravity == Vec3::ZERO {
        Vec3::new(0.0, -9.8, 0.0)
    } else {
        cloth.gravity
    };
    let damping = if cloth.damping == 0.0 {
        0.03
    } else {
        cloth.damping
    };
    let stiffness = if cloth.stiffness == 0.0 {
        0.02
    } else {
        cloth.stiffness
    };
    let bending = if cloth.bending == 0.0 {
        0.5
    } else {
        cloth.bending
    };
    let thickness = if cloth.thickness == 0.0 {
        0.005
    } else {
        cloth.thickness
    };
    let colliders = place_colliders(&cloth.colliders, bones, matrix, thickness);

    let frame = s.schedule(dt);
    let back = matrix.inverse();

    if s.posed.len() != meshes.len() {
        s.posed = vec![PosedMesh::default(); meshes.len()];
    }
    for (i, m) in meshes.iter().enumerate() {
        if !m.skinned() {
            continue;
        }
        let n = m.vertex_count();
        let pm = &mut s.posed[i];
        if pm.positions.len() != n {
            pm.positions = vec![Vec3::ZERO; n];
            if !m.normals.is_empty() {
                pm.normals = vec![Vec3::ZERO; n];
            }
        }
        skin(
            &mut pm.positions,
            &m.positions,
            &m.joints,
            &m.weights,
            bones,
        );
        if !m.normals.is_empty() {
            for v in 0..n {
                pm.normals[v] = skin_normal(m.normals[v], &m.joints[v], &m.weights[v], bones)
                    .normalize_or_zero();
            }
        }

        if let Some(c) = s.meshes.get_mut(&i) {
            c.last_target.copy_from_slice(&c.target);
            for p in 0..c.first.len() {
                c.target[p] = matrix.transform_point3(pm.positions[c.first[p] as usize]);
            }
            if !pm.normals.is_empty() {
                for &p in &c.free {
                    c.normal[p as usize] = matrix
                        .transform_vector3(pm.normals[c.first[p as usize] as usize])
                        .normalize_or_zero();
                }
            }
            if !c.started || restart {
                c.pos.copy_from_slice(&c.target);
                c.prev.copy_from_slice(&c.target);
                c.last_target.copy_from_slice(&c.target);
                c.draw_prev.fill(Vec3::ZERO);
                c.draw_offset.fill(Vec3::ZERO);
                c.started = true;
            }
            c.reach(&colliders);
            for k in 0..frame.steps {
                let t = frame.first + k as f32 * frame.stride;
                c.draw_prev.copy_from_slice(&c.draw_offset);
                c.step(
                    CLOTH_STEP,
                    t,
                    gravity,
                    1.0 - damping,
                    stiffness,
                    bending,
                    &colliders,
                );
                for &p in &c.free {
                    let p = p as usize;
                    c.draw_offset[p] = c.pos[p] - c.target_at(p as i32, t);
                }
            }
            // Interpolate displacement, not world position: pinned and
            // loose vertices follow the same current pose even between
            // physics ticks.
            if !pm.normals.is_empty() {
                let mut pose_normal = std::mem::take(&mut c.pose_normal);
                c.surface_normals(&mut pose_normal, &pm.positions);
                c.pose_normal = pose_normal;
            }
            for i in 0..c.free.len() {
                let p = c.free[i] as usize;
                c.draw_position[p] = back.transform_point3(c.draw_near(i, frame.alpha, &colliders));
            }
            for (v, &p) in c.particle.iter().enumerate() {
                if c.freedom[p as usize] > 0.0 {
                    pm.positions[v] = c.draw_position[p as usize];
                }
            }
            if !pm.normals.is_empty() {
                let positions = pm.positions.clone();
                c.deform_normals(&positions, &mut pm.normals);
            }
        }
    }
}

/// cloth_freedom is a helper for making ClothMeshSpec.freedom: for each
/// vertex, rate times its distance along the mesh from the nearest pinned
/// one, capped at most. pinned says which vertices are held; with none,
/// nothing moves. Vertices at the same place count as one.
pub fn cloth_freedom(
    vertices: &[Vec3],
    triangles: &[i32],
    pinned: &[bool],
    rate: f32,
    most: f32,
) -> Vec<f32> {
    let c = ClothMesh::new(vertices, triangles, &[]);
    let n = c.first.len();
    // Every edge this time, pinned or not.
    let mut adj: Vec<Vec<i32>> = vec![vec![]; n];
    let mut t = 0;
    while t + 2 < triangles.len() {
        for k in 0..3 {
            let (a, b) = (
                c.particle[triangles[t + k] as usize],
                c.particle[triangles[t + (k + 1) % 3] as usize],
            );
            if a != b {
                adj[a as usize].push(b);
                adj[b as usize].push(a);
            }
        }
        t += 3;
    }
    let mut dist = vec![f32::INFINITY; n];
    let mut queue = DistQueue::default();
    for (v, pin) in pinned.iter().enumerate() {
        let p = c.particle[v];
        if *pin && dist[p as usize] != 0.0 {
            dist[p as usize] = 0.0;
            queue.push(p, 0.0);
        }
    }
    while queue.len() > 0 {
        let (p, d) = queue.pop();
        if d > dist[p as usize] {
            continue;
        }
        for &q in &adj[p as usize] {
            let nd = d + vertices[c.first[p as usize] as usize]
                .distance(vertices[c.first[q as usize] as usize]);
            if nd < dist[q as usize] {
                dist[q as usize] = nd;
                queue.push(q, nd);
            }
        }
    }
    let mut out = vec![0.0; vertices.len()];
    for (v, &p) in c.particle.iter().enumerate() {
        if dist[p as usize].is_finite() {
            out[v] = most.min(rate * dist[p as usize]);
        }
    }
    out
}

/// DistQueue is a min-heap of particles by distance, for cloth_freedom.
#[derive(Default)]
struct DistQueue {
    p: Vec<i32>,
    d: Vec<f32>,
}

impl DistQueue {
    fn len(&self) -> usize {
        self.p.len()
    }

    fn push(&mut self, p: i32, d: f32) {
        self.p.push(p);
        self.d.push(d);
        let mut i = self.p.len() - 1;
        while i > 0 {
            let parent = (i - 1) / 2;
            if self.d[parent] <= self.d[i] {
                break;
            }
            self.swap(i, parent);
            i = parent;
        }
    }

    fn pop(&mut self) -> (i32, f32) {
        let (p, d) = (self.p[0], self.d[0]);
        let last = self.p.len() - 1;
        self.swap(0, last);
        self.p.truncate(last);
        self.d.truncate(last);
        let mut i = 0;
        loop {
            let (l, r, mut small) = (2 * i + 1, 2 * i + 2, i);
            if l < last && self.d[l] < self.d[small] {
                small = l;
            }
            if r < last && self.d[r] < self.d[small] {
                small = r;
            }
            if small == i {
                break;
            }
            self.swap(i, small);
            i = small;
        }
        (p, d)
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.p.swap(i, j);
        self.d.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// strip is a vertical ribbon of quads hanging from its top two
    /// vertices, with the bottom row's vertices duplicated (as a texture
    /// seam splits them).
    fn strip(rows: usize) -> (Vec<Vec3>, Vec<i32>) {
        let mut v = vec![];
        for r in 0..=rows {
            let y = -(r as f32) * 0.1;
            v.push(Vec3::new(0.0, y, 0.0));
            v.push(Vec3::new(0.1, y, 0.0));
        }
        let mut t = vec![];
        for r in 0..rows {
            let a = 2 * r as i32;
            t.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
        }
        // The seam: the last row again, used by no triangle.
        v.push(v[2 * rows]);
        v.push(v[2 * rows + 1]);
        (v, t)
    }

    // render/cloth_test.go TestClothMeshWeldsSeams.
    #[test]
    fn welds_seams() {
        let (v, tri) = strip(2);
        let mut freedom = vec![0.0; v.len()];
        *freedom.last_mut().unwrap() = 0.3;
        let c = ClothMesh::new(&v, &tri, &freedom);
        assert_eq!(c.first.len(), 6);
        let corner = c.particle[5];
        assert!(c.particle[v.len() - 1] == corner && c.freedom[corner as usize] == 0.3);
        assert_eq!(c.free, vec![corner]);
    }

    /// hang steps c at rest (targets where the vertices are, offset by
    /// shift) for n frames of one step each.
    fn hang(c: &mut ClothMesh, v: &[Vec3], shift: Vec3, n: usize, colliders: &[WorldCapsule]) {
        for p in 0..c.first.len() {
            c.target[p] = v[c.first[p] as usize] + shift;
        }
        if !c.started {
            c.pos.copy_from_slice(&c.target);
            c.prev.copy_from_slice(&c.target);
            c.last_target.copy_from_slice(&c.target);
            c.started = true;
        }
        for _ in 0..n {
            c.step(
                CLOTH_STEP,
                1.0,
                Vec3::new(0.0, -9.8, 0.0),
                0.97,
                0.02,
                0.5,
                colliders,
            );
            c.last_target.copy_from_slice(&c.target);
        }
    }

    // TestClothStaysWithinFreedom.
    #[test]
    fn stays_within_freedom() {
        let (v, tri) = strip(4);
        let freedom = cloth_freedom(&v, &tri, &[true, true], 0.5, 0.2);
        let mut c = ClothMesh::new(&v, &tri, &freedom);
        hang(&mut c, &v, Vec3::ZERO, 1, &[]);
        hang(&mut c, &v, Vec3::new(0.5, 0.0, 0.0), 1, &[]);
        let bottom = c.particle[8] as usize;
        let target = c.target[bottom];
        let lag = c.pos[bottom].distance(target);
        assert!(lag != 0.0, "the free end moved with the body at once");
        assert!(
            lag <= c.freedom[bottom] + 1e-4,
            "lag {lag} beyond freedom {}",
            c.freedom[bottom]
        );
        for p in 0..c.first.len() {
            if c.freedom[p] == 0.0 {
                assert_eq!(
                    c.at(p as i32, 1.0),
                    c.target[p],
                    "pinned particle {p} moved"
                );
            }
        }
        hang(&mut c, &v, Vec3::new(0.5, 0.0, 0.0), 600, &[]);
        let d = c.pos[bottom].distance(target);
        assert!(
            d <= 0.05,
            "after settling the free end is {d} from its place"
        );
    }

    // TestClothKeepsItsEdges.
    #[test]
    fn keeps_its_edges() {
        let (v, tri) = strip(4);
        let mut c = ClothMesh::new(&v, &tri, &cloth_freedom(&v, &tri, &[true, true], 1.0, 1.0));
        hang(&mut c, &v, Vec3::ZERO, 1, &[]);
        hang(&mut c, &v, Vec3::new(0.0, 0.0, 0.3), 30, &[]);
        for e in &c.edges {
            let got = c.at(e[0], 1.0).distance(c.at(e[1], 1.0));
            let want = c.target[e[0] as usize].distance(c.target[e[1] as usize]);
            assert!(
                (got - want).abs() <= 0.02,
                "edge {e:?} is {got} long, want about {want}"
            );
        }
    }

    // TestCollidePushesOut.
    #[test]
    fn collide_pushes_out() {
        let k = [WorldCapsule {
            a: Vec3::ZERO,
            b: Vec3::Y,
            radius: 0.1,
        }];
        let got = collide(Vec3::new(0.05, 0.5, 0.0), Vec3::new(0.2, 0.5, 0.0), &k);
        assert!((got.x - 0.1).abs() <= 1e-5 && got.y == 0.5, "{got}");
        let got = collide(Vec3::new(0.02, 0.5, 0.0), Vec3::new(0.06, 0.5, 0.0), &k);
        assert!((got.x - 0.1).abs() <= 1e-5, "{got}");
        let got = collide(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 0.5, -0.3), &k);
        assert!((got.z + 0.1).abs() <= 1e-5, "{got}");
        let p = Vec3::new(0.3, 0.5, 0.0);
        assert_eq!(collide(p, p, &k), p);
    }

    // TestClothFreedomGrowsAlongTheMesh.
    #[test]
    fn freedom_grows_along_the_mesh() {
        let (v, tri) = strip(3);
        let f = cloth_freedom(&v, &tri, &[true, true], 0.5, 0.12);
        assert!(f[0] == 0.0 && f[1] == 0.0);
        assert!((f[2] - 0.05).abs() <= 1e-5, "one row down: {}", f[2]);
        assert!(f[6] == 0.12 && f[v.len() - 2] == 0.12);
        let none = cloth_freedom(&v, &tri, &[], 1.0, 1.0);
        assert_eq!(none[6], 0.0);
    }

    // TestPlaceColliders.
    #[test]
    fn place_colliders_with_bones() {
        let up = Mat4::from_translation(Vec3::Y);
        let bones = [Mat4::IDENTITY, up];
        let got = place_colliders(
            &[
                Capsule {
                    a: Vec3::X,
                    b: Vec3::X * 2.0,
                    bone_a: 0,
                    bone_b: 1,
                    radius: 0.1,
                },
                Capsule {
                    bone_a: 5,
                    ..Default::default()
                },
            ],
            &bones,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0)),
            0.01,
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].a, Vec3::new(1.0, 0.0, 3.0));
        assert_eq!(got[0].b, Vec3::new(2.0, 1.0, 3.0));
        assert!((got[0].radius - 0.11).abs() <= 1e-6);
    }

    // TestClothNeverGoesBehindItsPlace.
    #[test]
    fn never_goes_behind_its_place() {
        let (v, tri) = strip(4);
        let mut c = ClothMesh::new(&v, &tri, &cloth_freedom(&v, &tri, &[true, true], 1.0, 1.0));
        for &p in &c.free.clone() {
            c.normal[p as usize] = Vec3::Z; // the surface faces +Z
        }
        hang(&mut c, &v, Vec3::ZERO, 1, &[]);
        for i in 0..20 {
            hang(&mut c, &v, Vec3::new(0.0, 0.0, 0.05 * i as f32), 1, &[]);
            for &p in &c.free {
                let d = c.pos[p as usize].z - c.target[p as usize].z;
                assert!(
                    d >= -1e-5,
                    "frame {i}: particle {p} is {} behind its place",
                    -d
                );
            }
        }
    }

    // TestClothIsPushedOutOfTheBody.
    #[test]
    fn pushed_out_of_the_body() {
        let (v, tri) = strip(4);
        let mut c = ClothMesh::new(&v, &tri, &cloth_freedom(&v, &tri, &[true, true], 1.0, 1.0));
        let leg = [WorldCapsule {
            a: Vec3::new(0.05, -0.5, -0.1),
            b: Vec3::new(0.05, -0.5, 0.1),
            radius: 0.08,
        }];
        hang(&mut c, &v, Vec3::ZERO, 1, &leg);
        hang(&mut c, &v, Vec3::ZERO, 60, &leg);
        for &p in &c.free {
            let x = c.pos[p as usize];
            let d = x.distance(closest_on_segment(x, leg[0].a, leg[0].b));
            assert!(
                d >= leg[0].radius - 1e-4,
                "particle {p} is {} inside the leg",
                leg[0].radius - d
            );
        }
    }

    // TestClothRestsWhereItHangs.
    #[test]
    fn rests_where_it_hangs() {
        let (v, tri) = strip(4);
        let mut c = ClothMesh::new(&v, &tri, &cloth_freedom(&v, &tri, &[true, true], 1.0, 0.3));
        hang(&mut c, &v, Vec3::ZERO, 600, &[]);
        for p in 0..c.first.len() {
            let d = c.pos[p].distance(c.target[p]);
            assert!(d <= 0.002, "particle {p} rests {d} from where it hangs");
        }
    }

    // TestClothBendsBetweenTriangles.
    #[test]
    fn bends_between_triangles() {
        let (v, tri) = strip(2);
        let c = ClothMesh::new(&v, &tri, &cloth_freedom(&v, &tri, &[true, true], 1.0, 1.0));
        assert_eq!(c.bends.len(), 3, "{:?}", c.bends);
        for b in &c.bends {
            assert_ne!(b[0], b[1]);
        }
    }

    // TestReachFindsEveryColliderAParticleTouches.
    #[test]
    fn reach_finds_every_collider_a_particle_touches() {
        let mut vertices = vec![];
        let mut triangles = vec![];
        for i in 0..20 {
            let y = i as f32 * 0.05;
            vertices.push(Vec3::new(-0.05, y, 0.0));
            vertices.push(Vec3::new(0.05, y, 0.0));
            if i > 0 {
                let a = 2 * (i - 1);
                triangles.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
            }
        }
        let freedom = vec![0.08; vertices.len()];
        let mut c = ClothMesh::new(&vertices, &triangles, &freedom);
        c.target.copy_from_slice(&vertices);
        for p in 0..c.last_target.len() {
            c.last_target[p] = c.target[p] + Vec3::new(0.0, 0.0, 0.03);
        }
        let colliders = [
            WorldCapsule {
                a: Vec3::new(-0.3, 0.2, 0.02),
                b: Vec3::new(0.3, 0.25, 0.02),
                radius: 0.06,
            },
            WorldCapsule {
                a: Vec3::new(0.0, 0.5, -0.05),
                b: Vec3::new(0.0, 0.9, 0.05),
                radius: 0.07,
            },
            WorldCapsule {
                a: Vec3::new(2.0, 0.5, 0.0),
                b: Vec3::new(2.0, 0.9, 0.0),
                radius: 0.1,
            },
        ];
        c.reach(&colliders);
        assert!(!c.near.is_empty() && c.near.len() < c.free.len() * colliders.len());
        for (i, &p) in c.free.iter().enumerate() {
            let p = p as usize;
            for off in [
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 0.08),
                Vec3::new(0.0, 0.0, -0.08),
                Vec3::new(0.05, -0.05, 0.0),
                Vec3::new(-0.04, 0.0, 0.06),
            ] {
                for target in [c.target[p], c.last_target[p]] {
                    let x = target + off;
                    let (got, want) = (
                        c.collide(i, x, target, &colliders),
                        collide(x, target, &colliders),
                    );
                    assert_eq!(got, want, "particle {p} at {x}");
                }
            }
        }
    }

    #[test]
    fn schedule_owes_fixed_steps() {
        let mut s = ClothState::default();
        let f = s.schedule(1.0 / 60.0);
        assert_eq!(f.steps, 1);
        assert!(
            (f.first - 1.0).abs() < 1e-5 && f.alpha.abs() < 1e-5,
            "{f:?}"
        );
        // A slow frame runs at most CLOTH_MAX_STEPS and carries the rest.
        let f = s.schedule(0.1);
        assert_eq!(f.steps, CLOTH_MAX_STEPS);
    }
}
