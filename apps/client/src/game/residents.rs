//! game/residents.go: nearby characters, lightweight resident data elsewhere.
use super::Screen;
use crate::{
    character::{self, CharacterController, CharacterPhysics, Health, Intent, LifeState, Player},
    presentation::{Outfit, Roster},
};
use bevy::prelude::*;

pub const EMBODY_RADIUS: f32 = crate::landfall::cull_people::PEOPLE_SIGHT + 10.;
pub const RELEASE_RADIUS: f32 = EMBODY_RADIUS + 10.;
pub const MAX_EMBODIED: usize = 400;
pub const EMBODY_SLACK: usize = 20;
pub const EMBODY_BATCH: usize = 16;
const SLICES: usize = 8;
const WALK: f32 = 1.2;

#[derive(Clone, Debug, Default)]
pub struct Resident {
    pub feet: Vec3,
    pub skin: usize,
    pub outfit: Outfit,
    pub health: Health,
    pub home: Vec3,
    pub target: Vec3,
    pub left: f32,
    pub step: usize,
}
impl Resident {
    fn think(&mut self, index: usize, dt: f32, valid: impl FnOnce(Vec3) -> bool) {
        self.left -= dt;
        if self.left > 0. {
            return;
        }
        self.step += 1;
        self.left = 4. + (index % 5) as f32;
        let angle = (index + self.step * 3) as f64 * 2.399_963_229_728_653;
        let target = self.home + Vec3::new(angle.cos() as f32 * 1.5, 0., angle.sin() as f32 * 1.5);
        if valid(target) {
            self.target = target;
        }
    }
}
#[derive(Resource, Default)]
pub struct Residents {
    pub list: Vec<Resident>,
    pub near: usize,
    pub generation: u64,
    bodies: Vec<Option<Entity>>,
    order: Vec<(usize, f32)>,
    slice: usize,
}
impl Residents {
    pub fn clear(&mut self) {
        self.list.clear();
        self.generation += 1;
    }
    pub fn truncate(&mut self, n: usize) {
        self.list.truncate(n);
    }
}
#[derive(Component, Clone, Copy, Debug)]
pub struct ResidentOf {
    pub index: usize,
    pub generation: u64,
}
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResidentsSet;
pub struct ResidentsPlugin;
impl Plugin for ResidentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Residents>()
            .configure_sets(
                Update,
                ResidentsSet.before(character::CharacterSystems::Input),
            )
            .add_systems(
                Update,
                (embody, move_data, steer).chain().in_set(ResidentsSet),
            );
    }
}
pub fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn embody(
    mut commands: Commands,
    mut residents: ResMut<Residents>,
    roster: Res<Roster>,
    characters: Query<(
        Entity,
        &ResidentOf,
        &Transform,
        &Health,
        Option<&CharacterController>,
    )>,
    players: Query<&Transform, With<Player>>,
    screen: Res<State<Screen>>,
) {
    let rs = &mut *residents;
    rs.bodies.clear();
    rs.bodies.resize(rs.list.len(), None);
    for (e, of, tr, health, cc) in &characters {
        if of.generation != rs.generation
            || of.index >= rs.list.len()
            || rs.bodies[of.index].is_some()
        {
            commands.entity(e).despawn();
            continue;
        }
        rs.bodies[of.index] = Some(e);
        rs.list[of.index].feet = tr.translation - Vec3::Y * cc.map_or(0.3, |c| c.height / 2.);
        rs.list[of.index].health = *health;
    }
    let player = players.single().ok();
    if player.is_none() || roster.skins.is_empty() || *screen.get() != Screen::Playing {
        rs.near = rs.bodies.iter().flatten().count();
        return;
    }
    let player = player.unwrap();
    rs.order.clear();
    for (i, r) in rs.list.iter().enumerate() {
        let far = flat_distance(r.feet, player.translation);
        if far < RELEASE_RADIUS {
            rs.order.push((i, far));
        } else if let Some(e) = rs.bodies[i].take() {
            commands.entity(e).despawn();
        }
    }
    rs.order.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut made = 0;
    rs.near = 0;
    for (rank, &(index, far)) in rs.order.iter().enumerate() {
        let entity = rs.bodies[index];
        let keep = entity.is_some() && rank < MAX_EMBODIED + EMBODY_SLACK;
        let want = rank < MAX_EMBODIED && far < EMBODY_RADIUS;
        if keep || (entity.is_some() && want) {
            rs.near += 1;
        } else if let Some(e) = entity {
            commands.entity(e).despawn();
            rs.bodies[index] = None;
        } else if want && made < EMBODY_BATCH {
            let who = &rs.list[index];
            // A stable idle phase replaces Go's random offset without synchronizing the crowd.
            let e = roster.spawn(
                &mut commands,
                who.skin,
                who.feet + Vec3::Y * 0.05,
                ((index * 37) % 101) as f32 / 101.,
            );
            commands.entity(e).insert((
                who.outfit,
                ResidentOf {
                    index,
                    generation: rs.generation,
                },
                crate::presentation::cloth::Rigid,
            ));
            if who.health.state != LifeState::Healthy {
                let at = Transform::from_translation(who.feet + Vec3::Y * 0.3)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
                character::knock_down(&mut commands.entity(e), at, who.health, Vec3::ZERO);
            }
            made += 1;
            rs.near += 1;
        }
    }
}
fn move_data(mut residents: ResMut<Residents>, clock: Res<Time>, screen: Res<State<Screen>>) {
    if *screen.get() != Screen::Playing || residents.list.is_empty() {
        return;
    }
    let rs = &mut *residents;
    let dt = clock.delta_secs() * SLICES as f32;
    rs.slice = (rs.slice + 1) % SLICES;
    for i in (rs.slice..rs.list.len()).step_by(SLICES) {
        if rs.bodies.get(i).is_some_and(Option::is_some) {
            continue;
        }
        let r = &mut rs.list[i];
        if r.health.state != LifeState::Healthy {
            continue;
        }
        r.think(i, dt, |_| true);
        let to = (r.target - r.feet) * Vec3::new(1., 0., 1.);
        let d = to.length();
        if d > 0.25 {
            r.feet += to * (WALK * dt / d).min(1.);
        }
    }
}
fn steer(
    mut actors: Query<(Entity, &ResidentOf, &mut Intent, &Transform)>,
    mut residents: ResMut<Residents>,
    screen: Res<State<Screen>>,
    clock: Res<Time>,
    physics: CharacterPhysics,
) {
    for (e, of, mut intent, tr) in &mut actors {
        *intent = default();
        if *screen.get() != Screen::Playing || of.generation != residents.generation {
            continue;
        }
        let Some(r) = residents.list.get_mut(of.index) else {
            continue;
        };
        if r.health.state != LifeState::Healthy {
            continue;
        }
        r.think(of.index, clock.delta_secs(), |at| {
            !physics.overlap_capsule_excluding(at + Vec3::Y * 0.95, 0.35, 1.8, e)
        });
        let dir = (r.target - tr.translation) * Vec3::new(1., 0., 1.);
        if dir.length() > 0.25 {
            intent.move_dir = dir.normalize();
        }
    }
}
