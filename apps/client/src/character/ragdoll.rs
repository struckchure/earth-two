//! Hybrid torso/particle ragdoll: direct port of character/ragdoll.go.
use super::{
    CharacterPhysics, RayHit,
    contacts::{BoneInfo, BoneTransform, limb_rotation, rotate_branch},
    health::{RAGDOLL_HEIGHT, RAGDOLL_OFFSET, RAGDOLL_RADIUS},
};
use avian3d::dynamics::rigid_body::forces::{ReadRigidBodyForces, WriteRigidBodyForces};
use avian3d::prelude::*;
use bevy::prelude::*;

pub const CONTACT_SKIN: f32 = 0.002;
#[derive(Component, Debug)]
pub struct Ragdoll {
    pub velocity: Vec3,
    pub rig: Option<RagdollRig>,
    pub quiet: f32,
    pub ground_quiet: f32,
}
impl Ragdoll {
    pub fn new(velocity: Vec3) -> Self {
        Self {
            velocity,
            rig: None,
            quiet: 0.,
            ground_quiet: 0.,
        }
    }
}
#[derive(Debug)]
struct Point {
    bone: usize,
    position: Vec3,
    previous: Vec3,
    drawn: Vec3,
    local: Vec3,
    radius: f32,
    weight: f32,
}
#[derive(Clone, Copy, Debug)]
struct Link {
    a: usize,
    b: usize,
    min: f32,
    max: f32,
    bone: bool,
}
#[derive(Debug)]
pub struct RagdollRig {
    bones: Vec<BoneInfo>,
    rest: Vec<BoneTransform>,
    points: Vec<Point>,
    links: Vec<Link>,
    root: Transform,
    step_time: f32,
}
impl RagdollRig {
    pub fn new(
        bones: &[BoneInfo],
        pose: &[BoneTransform],
        matrix: Mat4,
        root: Transform,
        velocity: Vec3,
    ) -> Option<Self> {
        if bones.is_empty() || pose.len() != bones.len() {
            return None;
        }
        let mut rig = Self {
            bones: bones.to_vec(),
            rest: pose.to_vec(),
            points: vec![],
            links: vec![],
            root,
            step_time: 1. / 120.,
        };
        for chain in [
            &["neck_01", "head"][..],
            &["upperarm_l", "lowerarm_l", "hand_l", "middle_03_l"],
            &["upperarm_r", "lowerarm_r", "hand_r", "middle_03_r"],
            &["thigh_l", "calf_l", "foot_l", "ball_l"],
            &["thigh_r", "calf_r", "foot_r", "ball_r"],
        ] {
            let mut nodes: Vec<usize> = vec![];
            for (i, name) in chain.iter().enumerate() {
                let Some(bone) = bones.iter().position(|b| b.name == *name) else {
                    break;
                };
                let radius = if *name == "head" {
                    0.13
                } else if chain[0].starts_with("thigh_") {
                    0.085
                } else {
                    0.065
                };
                let position = matrix.transform_point3(pose[bone].translation);
                let n = rig.points.len();
                rig.points.push(Point {
                    bone,
                    position,
                    previous: position,
                    drawn: position,
                    local: root.to_matrix().inverse().transform_point3(position),
                    radius,
                    weight: if i == 0 { 0. } else { 1. },
                });
                if let Some(&last) = nodes.last() {
                    let length = position.distance(rig.points[last].position);
                    rig.links.push(Link {
                        a: last,
                        b: n,
                        min: length,
                        max: length,
                        bone: true,
                    });
                }
                nodes.push(n);
            }
            if nodes.len() >= 3 {
                let a = rig.links[rig.links.len() - (nodes.len() - 1)].max;
                let b = rig.links[rig.links.len() - (nodes.len() - 2)].max;
                rig.links.push(Link {
                    a: nodes[0],
                    b: nodes[2],
                    min: ((a - b).abs() + 0.01).max((a + b) * 0.28),
                    max: (a + b) * 0.995,
                    bone: false,
                });
            }
        }
        for p in &mut rig.points {
            let name = &bones[p.bone].name;
            let momentum = velocity
                * if ["calf_", "foot_", "ball_"]
                    .iter()
                    .any(|n| name.starts_with(n))
                {
                    1.25
                } else {
                    1.
                };
            p.previous = p.position - momentum * rig.step_time;
        }
        Some(rig)
    }
    pub fn point_count(&self) -> usize {
        self.points.len()
    }
    fn constrain(&mut self, link: Link) {
        let (a, b) = (&self.points[link.a], &self.points[link.b]);
        let delta = b.position - a.position;
        let d = delta.length();
        let weight = a.weight + b.weight;
        if d < 0.000001 || weight == 0. {
            return;
        }
        let correction = delta * ((d - d.clamp(link.min, link.max)) / (d * weight));
        let (wa, wb) = (a.weight, b.weight);
        self.points[link.a].position += correction * wa;
        self.points[link.b].position -= correction * wb;
    }
    pub fn step(
        &mut self,
        root: Transform,
        gravity: Vec3,
        dt: f32,
        sweep: &dyn Fn(Vec3, Vec3, f32) -> Option<RayHit>,
    ) {
        if dt <= 0. {
            return;
        }
        for p in &mut self.points {
            p.drawn = p.position;
        }
        let steps = (dt / (1. / 120.)).ceil().max(1.) as usize;
        let h = dt / steps as f32;
        for sub in 0..steps {
            let t = (sub + 1) as f32 / steps as f32;
            let at = Transform {
                translation: self.root.translation.lerp(root.translation, t),
                rotation: self.root.rotation.slerp(root.rotation, t),
                ..root
            };
            for p in &mut self.points {
                if p.weight == 0. {
                    p.position = at.transform_point(p.local);
                    p.previous = p.position;
                    continue;
                }
                let motion = (p.position - p.previous) * (h / self.step_time * (-2. * h).exp());
                p.previous = p.position;
                p.position += motion + gravity * h * h;
            }
            for _ in 0..8 {
                for i in 0..self.links.len() {
                    self.constrain(self.links[i]);
                }
                for p in &mut self.points {
                    if p.weight == 0. {
                        continue;
                    }
                    let delta = p.position - p.previous;
                    if let Some(hit) = sweep(p.previous, delta, p.radius) {
                        let d = delta.length();
                        if d > 0.000001 {
                            p.position = p.previous
                                + delta * (hit.distance / d).clamp(0., 1.)
                                + hit.normal * CONTACT_SKIN;
                        }
                    }
                }
            }
            for p in &mut self.points {
                if p.weight == 0. {
                    continue;
                }
                if let Some(hit) = sweep(p.position, Vec3::NEG_Y * 0.008, p.radius) {
                    let mut motion = p.position - p.previous;
                    motion -= hit.normal * motion.dot(hit.normal).min(0.);
                    p.previous = p.position - motion * 0.65;
                }
            }
            self.step_time = h;
        }
        self.root = root;
    }
    pub fn pose(&self, matrix: Mat4, alpha: f32) -> Vec<BoneTransform> {
        let mut pose = self.rest.clone();
        let inverse = matrix.inverse();
        for link in self.links.iter().filter(|l| l.bone) {
            let (a, b) = (&self.points[link.a], &self.points[link.b]);
            let point = |p: &Point| {
                inverse.transform_point3(if p.weight == 0. {
                    p.position
                } else {
                    p.drawn.lerp(p.position, alpha.clamp(0., 1.))
                })
            };
            let from = pose[b.bone].translation - pose[a.bone].translation;
            let to = point(b) - point(a);
            rotate_branch(&mut pose, &self.bones, a.bone, limb_rotation(from, to));
        }
        pose
    }
}

/// The same ground-only rolling resistance as Go, before Avian integrates
/// forces. Use non-waking forces so settling does not reset the sleep timer.
#[allow(clippy::type_complexity)]
pub fn resist_rolling(
    mut commands: Commands,
    mut bodies: Query<(
        Entity,
        &Position,
        &Rotation,
        &Mass,
        &mut Ragdoll,
        Forces,
        Has<Sleeping>,
    )>,
    physics: CharacterPhysics,
    gravity: Res<Gravity>,
    clock: Res<Time>,
    physics_clock: Res<Time<Physics>>,
) {
    if physics_clock.is_paused() {
        return;
    }
    for (e, at, rotation, mass, mut ragdoll, mut forces, asleep) in &mut bodies {
        if asleep {
            continue;
        }
        let quiet = ragdoll.ground_quiet;
        ragdoll.ground_quiet = 0.;
        let axis = rotation.0 * Vec3::Y;
        let centre = at.0 + axis * RAGDOLL_OFFSET;
        let linear = forces.linear_velocity();
        let angular = forces.angular_velocity();
        for along in [
            0.,
            RAGDOLL_HEIGHT / 2. - RAGDOLL_RADIUS,
            -(RAGDOLL_HEIGHT / 2. - RAGDOLL_RADIUS),
        ] {
            let Some(hit) = physics.cast_ray_excluding(
                centre + axis * along,
                Vec3::NEG_Y,
                (RAGDOLL_RADIUS + 0.02) / 0.82,
                e,
            ) else {
                continue;
            };
            if hit.normal.y < 0.82
                || hit.distance * hit.normal.y > RAGDOLL_RADIUS + 0.015
                || !physics.static_surface(hit.entity)
            {
                continue;
            }
            let normal_speed = linear.dot(hit.normal);
            if axis.dot(hit.normal).abs() > 0.35 || normal_speed > 0.5 {
                break;
            }
            let slide = linear - hit.normal * normal_speed;
            if linear.length_squared() < 0.08 * 0.08 && angular.length_squared() < 0.2 * 0.2 {
                ragdoll.ground_quiet = quiet + clock.delta_secs();
                if ragdoll.ground_quiet >= 0.35 {
                    commands.entity(e).insert(Sleeping);
                    break;
                }
            }
            let mut force = slide * (-8. * mass.0);
            if slide.length_squared() < 0.3 * 0.3 && angular.length_squared() < 1.5 * 1.5 {
                force -= (gravity.0 - hit.normal * gravity.0.dot(hit.normal)) * mass.0;
            }
            forces.non_waking().apply_force(force);
            forces
                .non_waking()
                .apply_torque(angular * (-8. * mass.0 * RAGDOLL_RADIUS * RAGDOLL_RADIUS));
            break;
        }
    }
}

pub fn step_ragdolls(
    mut bodies: Query<(Entity, &Position, &Rotation, &mut Ragdoll, Has<Sleeping>)>,
    physics: CharacterPhysics,
    gravity: Res<Gravity>,
    clock: Res<Time>,
    physics_clock: Res<Time<Physics>>,
) {
    if physics_clock.is_paused() {
        return;
    }
    for (e, at, rotation, mut ragdoll, asleep) in &mut bodies {
        if !asleep {
            ragdoll.quiet = 0.;
        }
        if asleep && ragdoll.quiet >= 0.35 {
            continue;
        }
        let Some(rig) = &mut ragdoll.rig else {
            continue;
        };
        rig.step(
            Transform::from_translation(at.0).with_rotation(rotation.0),
            gravity.0,
            clock.delta_secs(),
            &|from, delta, radius| {
                physics.sweep_capsule_excluding(from, delta, radius, 2. * radius + CONTACT_SKIN, e)
            },
        );
        if asleep {
            if rig
                .points
                .iter()
                .any(|p| p.position.distance_squared(p.drawn) > (2. * CONTACT_SKIN).powi(2))
            {
                ragdoll.quiet = 0.;
                continue;
            }
            ragdoll.quiet += clock.delta_secs();
            if ragdoll.quiet >= 0.35 {
                for p in &mut ragdoll.rig.as_mut().unwrap().points {
                    p.previous = p.position;
                    p.drawn = p.position;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
