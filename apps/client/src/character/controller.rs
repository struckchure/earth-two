//! A kinematic character controller on Avian, standing in for illusion's
//! Jolt `physics.CharacterController`: a capsule that slides along walls,
//! climbs slopes and steps, and sticks to the floor going down. The
//! numbers are Jolt's `CharacterVirtual` defaults (2 cm padding, 1 mm
//! collision tolerance, 10 cm predictive contacts, 50 cm floor stick, 2 cm
//! minimum stair step forward, 15 cm forward test) so thresholds the
//! traversal was tuned against still hold.
//!
//! Each fixed step, set [`CharacterController::walk`] to the desired
//! horizontal velocity; set `velocity.y` to jump. The step applies gravity
//! and writes `velocity`, `grounded` and `ground_normal` back. The entity's
//! `Transform` is the capsule's centre; moving it teleports the character.
//!
//! The controller also gives the entity a kinematic capsule collider, so
//! sensors, dynamic bodies and other characters' casts see it, as Jolt's
//! inner body did.

use avian3d::{collision::collider::contact_query, prelude::*};
use bevy::{ecs::system::SystemParam, prelude::*};

use super::CAPSULE_RADIUS;

/// How far the capsule keeps from geometry, so sweeps rarely start in
/// contact; and how much nearer than that counts as touching.
pub const PADDING: f32 = 0.02;
const COLLISION_TOLERANCE: f32 = 1.0e-3;
/// How far outside the capsule contacts are gathered, for support.
const PREDICTIVE_CONTACT: f32 = 0.1;
const MAX_COLLISION_ITERATIONS: usize = 5;
const MIN_TIME_REMAINING: f32 = 1.0e-4;
/// Leaving the ground going down (a step, a slope's brow), the capsule
/// is put back on it if it's within this.
const STICK_TO_FLOOR: f32 = 0.5;
const STAIRS_MIN_STEP_FORWARD: f32 = 0.02;
const STAIRS_STEP_FORWARD_TEST: f32 = 0.15;
/// How far from the capsule's centre an overlap has to reach before it
/// counts, as Jolt's narrow phase passed shallow touches over.
const OVERLAP_SLACK: f32 = 0.005;

/// CharacterController moves an entity as a capsule that slides along
/// walls, climbs slopes and steps. It uses the entity's Transform for its
/// position and needs no RigidBody or Collider of its own.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CharacterController {
    /// Radius and Height (total, including caps) size the capsule.
    pub radius: f32,
    pub height: f32,
    /// The steepest walkable slope in radians; 0 means 45°.
    pub max_slope: f32,
    /// The tallest step the character walks up; 0 means none.
    pub step_height: f32,

    /// The desired horizontal velocity (Y is ignored unless `controlled`).
    pub walk: Vec3,
    /// Disables gravity and ground snapping, and includes `walk.y`. Use for
    /// ladders and collision-tested traversal; false keeps normal walking.
    pub controlled: bool,
    /// The character's velocity after the last step.
    pub velocity: Vec3,
    /// Whether the character stood on walkable ground after the last step.
    pub grounded: bool,
    /// The normal of the ground under the character.
    pub ground_normal: Vec3,

    /// Every steps the character only once every that many fixed steps, by
    /// all the time since it last was: for a crowd, where those far from
    /// anyone watching needn't be stepped as often. 0 or 1 steps it every
    /// time. Characters are spread over the steps between.
    pub every: u32,
    /// Whether the last fixed step moved the character (always, unless
    /// `every` passed it over). `velocity`, `grounded` and `ground_normal`
    /// are as of the last step that did.
    pub stepped: bool,
}

impl Default for CharacterController {
    fn default() -> Self {
        CharacterController {
            radius: 0.0,
            height: 0.0,
            max_slope: 0.0,
            step_height: 0.0,
            walk: Vec3::ZERO,
            controlled: false,
            velocity: Vec3::ZERO,
            grounded: false,
            ground_normal: Vec3::ZERO,
            every: 0,
            stepped: false,
        }
    }
}

impl CharacterController {
    /// A standing character's controller: the capsule, and 30 cm steps.
    pub fn standing(radius: f32, height: f32, step_height: f32) -> Self {
        CharacterController {
            radius,
            height,
            step_height,
            ..Default::default()
        }
    }

    fn effective_radius(&self) -> f32 {
        if self.radius <= 0.0 {
            CAPSULE_RADIUS
        } else {
            self.radius
        }
    }

    fn effective_height(&self) -> f32 {
        self.height.max(2.0 * self.effective_radius() + 0.01)
    }

    fn slope(&self) -> f32 {
        if self.max_slope <= 0.0 {
            std::f32::consts::FRAC_PI_4
        } else {
            self.max_slope
        }
    }
}

/// The controller's own bookkeeping, inserted by [`prepare_characters`]:
/// the capsule it last built, Jolt's "supported" state (standing on
/// anything, steep or not), and the spread for `every`.
#[derive(Component, Clone, Debug, Default)]
pub struct ControllerState {
    pub built: Option<(f32, f32)>,
    pub supported: bool,
    ground_velocity: Vec3,
    turn: u32,
    owed: f32,
}

/// Where a ray or a sweep hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

/// The world as the character systems query it: illusion's
/// `physics.Physics` casts and overlaps on Avian's spatial queries.
#[derive(SystemParam)]
pub struct CharacterPhysics<'w, 's> {
    pub spatial: SpatialQuery<'w, 's>,
    bodies: Query<'w, 's, &'static RigidBody>,
    owners: Query<'w, 's, &'static ColliderOf>,
    sensors: Query<'w, 's, (), With<Sensor>>,
    colliders: Query<'w, 's, (&'static Position, &'static Rotation, &'static Collider)>,
}

fn capsule(radius: f32, height: f32) -> Collider {
    Collider::capsule(radius, (height - 2.0 * radius).max(0.002))
}

impl CharacterPhysics<'_, '_> {
    /// The body a collider belongs to: itself, or the body it's a child of.
    pub fn body_of(&self, entity: Entity) -> Entity {
        if self.bodies.contains(entity) {
            entity
        } else {
            self.owners.get(entity).map(|c| c.body).unwrap_or(entity)
        }
    }

    /// Whether `entity` is a solid, static body suitable for authored
    /// traversal. A collider with no body at all is static too, as Avian
    /// has it.
    pub fn static_surface(&self, entity: Entity) -> bool {
        let body = self.body_of(entity);
        if self.sensors.contains(entity) || self.sensors.contains(body) {
            return false;
        }
        self.bodies
            .get(body)
            .map(|rb| rb.is_static())
            .unwrap_or(true)
    }

    /// The closest entity along a ray from `origin` in `direction`, up to
    /// `max_distance` away, ignoring `exclude` (typically the caster).
    pub fn cast_ray_excluding(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        exclude: Entity,
    ) -> Option<RayHit> {
        let dir = Dir3::new(direction).ok()?;
        let filter = SpatialQueryFilter::from_excluded_entities([exclude]);
        let hit = self
            .spatial
            .cast_ray(origin, dir, max_distance, true, &filter)?;
        Some(RayHit {
            entity: hit.entity,
            point: origin + dir * hit.distance,
            normal: hit.normal,
            distance: hit.distance,
        })
    }

    /// Whether a vertical capsule penetrates anything but `exclude`, by more
    /// than a touch.
    pub fn overlap_capsule_excluding(
        &self,
        center: Vec3,
        radius: f32,
        height: f32,
        exclude: Entity,
    ) -> bool {
        if radius <= 0.0 || height <= 2.0 * radius {
            return true;
        }
        // The capsule shrunk by the slack is exactly the set of points
        // deeper than it inside the real one.
        let shape = capsule(radius - OVERLAP_SLACK, height - 2.0 * OVERLAP_SLACK);
        let filter = SpatialQueryFilter::from_excluded_entities([exclude]);
        let mut found = false;
        self.spatial
            .shape_intersections_callback(&shape, center, Quat::IDENTITY, &filter, |_| {
                found = true;
                false
            });
        found
    }

    /// The first obstruction of a vertical capsule moving by `delta`.
    /// Touching surfaces it moves along or away from don't count.
    pub fn sweep_capsule_excluding(
        &self,
        center: Vec3,
        delta: Vec3,
        radius: f32,
        height: f32,
        exclude: Entity,
    ) -> Option<RayHit> {
        self.sweep(&capsule(radius, height), center, delta, 0.0, exclude)
    }

    fn sweep(
        &self,
        shape: &Collider,
        center: Vec3,
        delta: Vec3,
        target_distance: f32,
        exclude: Entity,
    ) -> Option<RayHit> {
        let length = delta.length();
        if length * length < 1.0e-8 {
            return None;
        }
        let dir = Dir3::new(delta).ok()?;
        // Parry's casts to a target distance give unreliable normals for
        // a capsule gliding exactly that far over a floor, so the cast is
        // to contact and the padding is taken off afterwards.
        let config = ShapeCastConfig {
            max_distance: length,
            target_distance: 0.0,
            compute_contact_on_penetration: true,
            ignore_origin_penetration: true,
        };
        let filter = SpatialQueryFilter::from_excluded_entities([exclude]);
        // The nearest hit, but for touching surfaces the capsule moves
        // along or away from: a floor it slides over, a wall it leaves.
        let mut best: Option<ShapeHitData> = None;
        self.spatial.shape_hits_callback(
            shape,
            center,
            Quat::IDENTITY,
            dir,
            &config,
            &filter,
            |hit| {
                let touching = hit.distance <= 1.0e-4
                    && (hit.point2 - hit.point1).dot(hit.normal1) <= OVERLAP_SLACK;
                if touching && hit.normal1.dot(*dir) >= -1.0e-4 {
                    return true;
                }
                if best.as_ref().is_none_or(|b| hit.distance < b.distance) {
                    best = Some(hit);
                }
                true
            },
        );
        let hit = best?;
        // Stop short, so the capsule ends the padding off the surface.
        let backoff = target_distance / hit.normal1.dot(*dir).abs().max(1.0e-3);
        Some(RayHit {
            entity: hit.entity,
            point: hit.point1,
            normal: hit.normal1,
            distance: (hit.distance - backoff).max(0.0),
        })
    }

    /// Tests clearance and preserves the feet. The next step rebuilds the
    /// capsule.
    pub fn resize_character(
        &self,
        entity: Entity,
        cc: &mut CharacterController,
        transform: &mut Transform,
        height: f32,
    ) -> bool {
        if height <= 2.0 * cc.radius {
            return false;
        }
        let mut center = transform.translation;
        center.y += (height - cc.height) / 2.0;
        if self.overlap_capsule_excluding(center, cc.radius, height, entity) {
            return false;
        }
        cc.height = height;
        transform.translation = center;
        true
    }

    /// Everything within `separation` of the capsule, as contacts: the
    /// point and outward normal on the other shape, and the gap (negative
    /// when penetrating) less the padding, as Jolt reports its contacts.
    fn contacts(
        &self,
        entity: Entity,
        radius: f32,
        height: f32,
        center: Vec3,
        separation: f32,
        out: &mut Vec<Contact>,
    ) {
        out.clear();
        let reach = PADDING + separation;
        let probe = capsule(radius + reach, height + 2.0 * reach);
        let shape = capsule(radius, height);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let mut near = Vec::new();
        self.spatial.shape_intersections_callback(
            &probe,
            center,
            Quat::IDENTITY,
            &filter,
            |other| {
                near.push(other);
                true
            },
        );
        for other in near {
            let Ok((position, rotation, collider)) = self.colliders.get(other) else {
                continue;
            };
            let Ok(Some(contact)) = contact_query::contact(
                &shape,
                center,
                Quat::IDENTITY,
                collider,
                position.0,
                *rotation,
                reach,
            ) else {
                continue;
            };
            // Parry's points are in world space; Avian turns them into the
            // collider's frame without moving them, so turn them back.
            let rotation = rotation.0;
            out.push(Contact {
                entity: other,
                point: rotation * contact.local_point2,
                normal: rotation * contact.local_normal2,
                distance: -contact.penetration - PADDING,
                had_collision: false,
            });
        }
    }
}

/// A nearby surface, as Jolt's `CharacterVirtual::Contact`.
#[derive(Clone, Copy, Debug)]
struct Contact {
    entity: Entity,
    point: Vec3,
    /// The surface normal, pointing at the character.
    normal: Vec3,
    /// The gap, less the padding: under the collision tolerance it touches.
    distance: f32,
    had_collision: bool,
}

/// What a step found under the character.
#[derive(Clone, Copy, Debug, Default)]
struct Support {
    supported: bool,
    ground_normal: Vec3,
    ground_velocity: Vec3,
}

/// The capsule the character is for the rest of the world: a kinematic
/// body with a capsule collider, rebuilt when the controller's size
/// changes (crouching, standing back up).
pub fn prepare_characters(
    mut commands: Commands,
    mut characters: Query<(Entity, &CharacterController, Option<&mut ControllerState>)>,
) {
    for (entity, cc, state) in &mut characters {
        let radius = cc.effective_radius();
        let height = cc.effective_height();
        let size = Some((radius, height));
        match state {
            Some(state) if state.built == size => {}
            Some(mut state) => {
                state.built = size;
                commands
                    .entity(entity)
                    .insert((RigidBody::Kinematic, capsule(radius, height)));
            }
            None => {
                commands.entity(entity).insert((
                    ControllerState {
                        built: size,
                        ..Default::default()
                    },
                    RigidBody::Kinematic,
                    capsule(radius, height),
                ));
            }
        }
    }
}

/// Moves every character by its `walk` and `velocity`, as illusion's
/// `moveCharacters` fed Jolt and `pullCharacters` read it back.
#[allow(clippy::type_complexity)]
pub fn step_characters(
    time: Res<Time>,
    physics_time: Res<Time<Physics>>,
    gravity: Res<Gravity>,
    physics: CharacterPhysics,
    mut characters: Query<(
        Entity,
        &mut CharacterController,
        &mut Transform,
        Option<&mut ControllerState>,
    )>,
) {
    let dt = time.delta_secs();
    if physics_time.is_paused() || dt <= 0.0 {
        return;
    }
    let g = gravity.0;
    let mut contacts = Vec::new();
    let mut scratch;
    for (entity, mut cc, mut transform, state) in &mut characters {
        let state: &mut ControllerState = match state {
            Some(state) => state.into_inner(),
            None => {
                scratch = ControllerState::default();
                &mut scratch
            }
        };
        if state.turn == 0 {
            // Spread characters stepped every so often over the steps.
            state.turn = entity.index_u32();
        }
        state.turn = state.turn.wrapping_add(1);
        state.owed += dt;
        cc.stepped = cc.every <= 1 || state.turn.is_multiple_of(cc.every);
        if !cc.stepped {
            continue;
        }
        let dt = state.owed;
        state.owed = 0.0;

        let mut v = cc.velocity;
        v.x = cc.walk.x;
        v.z = cc.walk.z;
        if cc.controlled {
            v.y = cc.walk.y;
        } else if cc.grounded && v.y <= 0.0 {
            v.y = state.ground_velocity_y();
        } else {
            v += g * dt;
        }

        let body = Capsule {
            entity,
            radius: cc.effective_radius(),
            height: cc.effective_height(),
            cos_slope: cc.slope().cos(),
        };
        let (position, support) = extended_update(
            &physics,
            &body,
            transform.translation,
            v,
            dt,
            cc.controlled,
            cc.step_height,
            state.supported,
            &mut contacts,
        );
        transform.translation = position;
        state.supported = support.supported;
        state.ground_velocity = support.ground_velocity;
        cc.velocity = v;
        cc.ground_normal = support.ground_normal;
        // Jolt considers steep-ground contact supported too. Public
        // `grounded` means walkable support, so a vertical wall cannot act
        // as a floor.
        cc.grounded = support.supported && support.ground_normal.y >= body.cos_slope - 0.001;
    }
}

impl ControllerState {
    fn ground_velocity_y(&self) -> f32 {
        self.ground_velocity.y
    }
}

/// The capsule being stepped.
struct Capsule {
    entity: Entity,
    radius: f32,
    height: f32,
    cos_slope: f32,
}

impl Capsule {
    fn shape(&self) -> Collider {
        capsule(self.radius, self.height)
    }

    fn steep(&self, normal: Vec3) -> bool {
        normal.y < self.cos_slope
    }
}

/// Jolt's `ExtendedUpdate`: move, find what supports the character, then
/// stick to the floor going off a step and walk up stairs. Controlled
/// characters (ladders, vaults) only move: no gravity, no snapping.
#[allow(clippy::too_many_arguments)]
fn extended_update(
    physics: &CharacterPhysics,
    body: &Capsule,
    mut position: Vec3,
    velocity: Vec3,
    dt: f32,
    controlled: bool,
    step_height: f32,
    was_supported: bool,
    contacts: &mut Vec<Contact>,
) -> (Vec3, Support) {
    let old = position;
    position = move_shape(physics, body, position, velocity, dt, contacts);
    let mut support = settle(physics, body, &mut position, velocity, contacts, None);

    if controlled {
        return (position, support);
    }

    // Going from supported to not supported, and not moving up: stick to
    // the floor.
    if was_supported && !support.supported && (position - old).y / dt <= 1.0e-6 {
        let down = Vec3::new(0.0, -STICK_TO_FLOOR, 0.0);
        if let Some(hit) = physics.sweep(&body.shape(), position, down, PADDING, body.entity) {
            position += down * (hit.distance / STICK_TO_FLOOR);
            support = settle(physics, body, &mut position, velocity, contacts, Some(hit));
        }
    }

    if step_height > 0.0 {
        let desired = super::horizontal(velocity * dt);
        let desired_len = desired.length();
        if desired_len > 0.0 {
            let forward = desired / desired_len;
            let achieved = super::horizontal(position - old).dot(forward).max(0.0);
            // If it didn't move as far as it wanted and it's against a slope
            // that's too steep, try the step up.
            if achieved + 1.0e-4 < desired_len
                && can_walk_stairs(body, support.supported, velocity, contacts)
            {
                let step_forward = forward * (desired_len - achieved).max(STAIRS_MIN_STEP_FORWARD);
                // Scan ahead along the ground normal in the horizontal plane,
                // unless that's too far from the way it's going.
                let mut test = super::direction(super::horizontal(-support.ground_normal));
                if test == Vec3::ZERO || test.dot(forward) < 75f32.to_radians().cos() {
                    test = forward;
                }
                let test = test * STAIRS_STEP_FORWARD_TEST;
                if let Some((at, found)) = walk_stairs(
                    physics,
                    body,
                    position,
                    velocity,
                    dt,
                    Vec3::new(0.0, step_height, 0.0),
                    step_forward,
                    test,
                    contacts,
                ) {
                    position = at;
                    support = found;
                }
            }
        }
    }
    (position, support)
}

/// Slides the capsule through the world along `velocity` for `dt`,
/// stopping the padding short of anything it hits and sliding along it,
/// a few times over.
fn move_shape(
    physics: &CharacterPhysics,
    body: &Capsule,
    mut position: Vec3,
    velocity: Vec3,
    dt: f32,
    contacts: &mut Vec<Contact>,
) -> Vec3 {
    let shape = body.shape();
    let mut v = velocity;
    let mut remaining = dt;
    let mut planes: Vec<Vec3> = Vec::new();
    contacts.clear();
    for _ in 0..MAX_COLLISION_ITERATIONS {
        if remaining < MIN_TIME_REMAINING {
            break;
        }
        let displacement = v * remaining;
        let length = displacement.length();
        if length * length < 1.0e-8 {
            break;
        }
        let Some(hit) = physics.sweep(&shape, position, displacement, PADDING, body.entity) else {
            position += displacement;
            break;
        };
        let moved = hit.distance.min(length);
        position += displacement * (moved / length);
        remaining -= remaining * (moved / length);
        let n = hit.normal;
        contacts.push(Contact {
            entity: hit.entity,
            point: hit.point,
            normal: n,
            distance: 0.0,
            had_collision: true,
        });
        // Slide along the plane; along the crease where two planes meet.
        let into = v.dot(n);
        if into < 0.0 {
            v -= n * into;
        }
        for p in &planes {
            if v.dot(*p) < -1.0e-6 {
                let crease = n.cross(*p);
                if crease.length_squared() > 1.0e-8 {
                    let crease = crease.normalize();
                    v = crease * v.dot(crease);
                }
            }
        }
        planes.push(n);
        if (displacement * (moved / length)).length_squared() < 1.0e-8 && planes.len() > 1 {
            break;
        }
    }
    position
}

/// Gathers the contacts around the capsule, pushes it out of anything it's
/// in, and works out what supports it, as Jolt's `UpdateSupportingContact`.
/// `touched` is a sweep's contact to count as colliding whether or not the
/// gather finds it (Jolt's `MoveToContact`).
fn settle(
    physics: &CharacterPhysics,
    body: &Capsule,
    position: &mut Vec3,
    velocity: Vec3,
    contacts: &mut Vec<Contact>,
    touched: Option<RayHit>,
) -> Support {
    let hit_planes: Vec<Contact> = contacts
        .iter()
        .copied()
        .filter(|c| c.had_collision)
        .collect();
    physics.contacts(
        body.entity,
        body.radius,
        body.height,
        *position,
        PREDICTIVE_CONTACT,
        contacts,
    );
    // Penetration recovery: out to the padding in one step.
    let mut push = Vec3::ZERO;
    for c in contacts.iter() {
        if c.distance < -COLLISION_TOLERANCE {
            push += c.normal * -c.distance;
        }
    }
    if push.length_squared() > 0.0 {
        *position += push;
        for c in contacts.iter_mut() {
            c.distance += push.dot(c.normal);
        }
    }
    // Flag contacts as colliding if they're close enough but ignore
    // contacts being moved away from. Those the sweep collided with stay.
    for c in contacts.iter_mut() {
        let swept = hit_planes.iter().any(|h| {
            physics.body_of(h.entity) == physics.body_of(c.entity) && h.normal.dot(c.normal) > 0.99
        }) || touched.is_some_and(|t| {
            physics.body_of(t.entity) == physics.body_of(c.entity) && t.normal.dot(c.normal) > 0.99
        });
        if c.distance < COLLISION_TOLERANCE && (swept || c.normal.dot(velocity) <= 1.0e-4) {
            c.had_collision = true;
        }
    }
    if let Some(t) = touched
        && !contacts
            .iter()
            .any(|c| c.had_collision && c.entity == t.entity)
    {
        contacts.push(Contact {
            entity: t.entity,
            point: t.point,
            normal: t.normal,
            distance: 0.0,
            had_collision: true,
        });
    }

    let mut num_supported = 0;
    let mut num_sliding = 0;
    let mut avg_normal = Vec3::ZERO;
    let mut num_avg = 0;
    let mut best: Option<Vec3> = None;
    let mut max_cos = f32::MIN;
    let mut deepest: Option<Vec3> = None;
    let mut smallest = f32::MAX;
    for c in contacts.iter().filter(|c| c.had_collision) {
        let cos = c.normal.y;
        if c.distance < smallest {
            deepest = Some(c.normal);
            smallest = c.distance;
        }
        // A contact above the supporting plane can't hold it up.
        if c.point.y - position.y - body.radius > 0.0 {
            continue;
        }
        if max_cos < cos {
            best = Some(c.normal);
            max_cos = cos;
        }
        if cos >= body.cos_slope {
            num_supported += 1;
        } else {
            num_sliding += 1;
        }
        // Less than 85° off up counts toward the average normal.
        if cos >= 0.08 {
            avg_normal += c.normal;
            num_avg += 1;
        }
    }
    let best = best.or(deepest);
    let ground_normal = if num_avg >= 1 {
        avg_normal.normalize()
    } else {
        best.unwrap_or(Vec3::ZERO)
    };
    Support {
        supported: num_supported > 0 || num_sliding > 0,
        ground_normal,
        // Only static geometry carries characters for now: platforms and
        // vehicles are not walked on.
        ground_velocity: Vec3::ZERO,
    }
}

/// Whether the character is supported and pushing into a slope too steep
/// to walk: a step's riser.
fn can_walk_stairs(body: &Capsule, supported: bool, velocity: Vec3, contacts: &[Contact]) -> bool {
    if !supported {
        return false;
    }
    let horizontal = super::horizontal(velocity);
    if horizontal.length_squared() < 1.0e-12 {
        return false;
    }
    contacts
        .iter()
        .any(|c| c.had_collision && c.normal.dot(horizontal) < 0.0 && body.steep(c.normal))
}

/// Jolt's `WalkStairs`: up, forward, and back down onto the step, if
/// there's a walkable floor there.
#[allow(clippy::too_many_arguments)]
fn walk_stairs(
    physics: &CharacterPhysics,
    body: &Capsule,
    position: Vec3,
    velocity: Vec3,
    dt: f32,
    step_up: Vec3,
    step_forward: Vec3,
    step_forward_test: Vec3,
    contacts: &mut Vec<Contact>,
) -> Option<(Vec3, Support)> {
    let shape = body.shape();
    let mut up = step_up;
    if let Some(hit) = physics.sweep(&shape, position, up, PADDING, body.entity) {
        let fraction = hit.distance / step_up.length();
        if fraction < 1.0e-6 {
            return None;
        }
        up *= fraction;
    }
    let up_position = position + up;

    // The steep slopes it would like to walk up.
    let character_velocity = step_forward / dt;
    let horizontal = super::horizontal(character_velocity);
    let steep: Vec<Vec3> = contacts
        .iter()
        .filter(|c| c.had_collision && c.normal.dot(horizontal) < 0.0 && body.steep(c.normal))
        .map(|c| c.normal)
        .collect();
    if steep.is_empty() {
        return None;
    }

    let mut scratch = Vec::new();
    let new_position = move_shape(
        physics,
        body,
        up_position,
        character_velocity,
        dt,
        &mut scratch,
    );
    let movement = new_position - up_position;
    let movement_sq = movement.length_squared();
    if movement_sq < 1.0e-8 {
        return None;
    }
    // It has to have made progress toward a steep slope, not just slid
    // along it.
    let max_dot = -0.05 * step_forward.length();
    if !steep.iter().any(|n| n.dot(movement) < max_dot) {
        return None;
    }

    let down = -up;
    let hit = physics.sweep(&shape, new_position, down, PADDING, body.entity)?;
    let mut floor = hit;
    if body.steep(hit.normal) {
        if step_forward_test.length_squared() < 1.0e-12 {
            return None;
        }
        // The edge of a step can give a normal that's too horizontal:
        // judge the floor further along instead.
        let test_position = move_shape(
            physics,
            body,
            up_position,
            step_forward_test / dt,
            dt,
            &mut scratch,
        );
        if (test_position - up_position).length_squared() <= movement_sq + 1.0e-8 {
            return None;
        }
        let test = physics.sweep(&shape, test_position, down, PADDING, body.entity)?;
        if body.steep(test.normal) {
            return None;
        }
        floor.normal = test.normal;
    }
    let mut at = new_position + down * (hit.distance / up.length());
    let mut support = settle(physics, body, &mut at, velocity, contacts, Some(hit));
    // On the ground, whatever the contact normal: the test found a floor.
    support.supported = true;
    if body.steep(support.ground_normal) {
        support.ground_normal = floor.normal;
    }
    Some((at, support))
}
