//! People are culled as the world's pieces are (`cull.rs`), but whole: a
//! body and everything it wears together, and in levels of detail, as a
//! crowd needs (`game/cull_people.go`). A person the renderer draws is
//! posed every frame, and one that's hidden isn't.
//!
//!   - In view out to [`PEOPLE_SIGHT`], a person's drawn; out of view but
//!     within [`PEOPLE_SHADOW`], only into the shadow map (so shadows don't
//!     pop as the camera turns); otherwise not at all.
//!   - Only those within [`PEOPLE_CAST`] cast shadows.
//!   - Past [`PEOPLE_NEAR`], a person is distant (posed in step with the
//!     rest of the crowd) and has no outline.
//!   - Physics steps a person less often the further off they are, and
//!     less often still out of view (`every`).
//!   - Only the [`CLOTH_BUDGET`] nearest in view, within [`CLOTH_RANGE`],
//!     have clothes that move with physics; everyone else's move with
//!     their skeleton alone.
//!
//! The player is always drawn in full. The people themselves are the
//! character port's: it gives this module a [`PeopleCull`] that lists the
//! bodies and applies each one's [`PersonDetail`] (its drawing, shadow,
//! distance, outline, cloth and step rate) the way its components want.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use super::cull::{CullEye, Drawn, View};

pub const PEOPLE_SIGHT: f32 = 45.0;
pub const PEOPLE_SHADOW: f32 = 20.0;
/// How far off a person still casts a shadow: past it, the shadow's small
/// and the shadow pass would pose them all over again.
pub const PEOPLE_CAST: f32 = 12.0;
pub const PEOPLE_NEAR: f32 = 10.0;
pub const CLOTH_RANGE: f32 = 8.0;
/// How many people's clothes move with physics at once. One who has them
/// keeps them until `CLOTH_SLACK` more are nearer, or they're `CLOTH_SLACK`
/// metres further off: starting cloth again builds it afresh.
pub const CLOTH_BUDGET: usize = 4;
pub const CLOTH_SLACK: usize = 2;
/// How often physics steps a person in view past `PEOPLE_NEAR`, further
/// than `PEOPLE_MID`, and out of view, in fixed steps.
pub const PEOPLE_MID: f32 = 25.0;
pub const STEPS_NEAR: u32 = 2;
pub const STEPS_FAR: u32 = 3;
pub const STEPS_OUT_OF_VIEW: u32 = 6;
/// A person's bounds about their middle.
pub const PERSON_RADIUS: f32 = 1.1;

/// A body the cull looks at: its entity, where its root stands (its
/// middle is 0.9 m up), and whether it's the player's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Person {
    pub body: Entity,
    pub at: Vec3,
    pub player: bool,
}

impl Person {
    pub fn center(&self) -> Vec3 {
        self.at + Vec3::new(0.0, 0.9, 0.0)
    }
}

/// How a person is to be drawn: as `cullPeople` sets it on their body and
/// what they wear. `every` is how often physics steps them, in fixed steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersonDetail {
    pub drawn: Drawn,
    pub casts: bool,
    pub distant: bool,
    pub cloth: bool,
    pub every: u32,
}

impl Default for PersonDetail {
    /// In full, as the player is.
    fn default() -> Self {
        PersonDetail {
            drawn: Drawn::Seen,
            casts: true,
            distant: false,
            cloth: true,
            every: 1,
        }
    }
}

/// Who gets cloth: the nearest in view, those who have it first among
/// equals (see [`CLOTH_SLACK`]).
pub fn clothed(v: &View, people: &[Person], had: &HashSet<Entity>) -> HashSet<Entity> {
    let mut near: Vec<(Entity, f32)> = Vec::new();
    for p in people {
        if p.player {
            continue;
        }
        let center = p.center();
        let far = center.distance(v.at);
        let mut reach = CLOTH_RANGE;
        if had.contains(&p.body) {
            reach += CLOTH_SLACK as f32;
        }
        if far < reach && v.sees(center, PERSON_RADIUS, PEOPLE_SIGHT) {
            near.push((p.body, far));
        }
    }
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut out = HashSet::new();
    for (i, (body, _)) in near.into_iter().enumerate() {
        if i < CLOTH_BUDGET || (had.contains(&body) && i < CLOTH_BUDGET + CLOTH_SLACK) {
            out.insert(body);
        }
    }
    out
}

/// `cullPeople`'s rules for one person: the player in full; anyone else by
/// how far off and whether in view, with `cloth` as [`clothed`] gave it.
pub fn detail(v: &View, p: &Person, cloth: bool) -> PersonDetail {
    if p.player {
        return PersonDetail::default();
    }
    let center = p.center();
    let far = center.distance(v.at);
    let drawn = if v.sees(center, PERSON_RADIUS, PEOPLE_SIGHT) {
        Drawn::Seen
    } else if far < PEOPLE_SHADOW {
        Drawn::ShadowOnly
    } else {
        Drawn::Unseen
    };
    let distant = far > PEOPLE_NEAR;
    let every = if drawn != Drawn::Seen {
        STEPS_OUT_OF_VIEW
    } else if far > PEOPLE_MID {
        STEPS_FAR
    } else if distant {
        STEPS_NEAR
    } else {
        1
    };
    PersonDetail {
        drawn,
        casts: far < PEOPLE_CAST,
        distant,
        cloth,
        every,
    }
}

/// The character port's side of the cull: which bodies there are, and how
/// to draw one as told. `apply` is called only when a person's detail
/// changes (or they're first seen), so it may insert and remove freely.
pub trait PeopleCull: Send + Sync + 'static {
    fn people(&self, world: &mut World) -> Vec<Person>;
    fn apply(&self, world: &mut World, person: &Person, detail: &PersonDetail);
}

/// The hook, once the character port installs one; without it the people
/// aren't culled.
#[derive(Resource)]
pub struct PeopleCuller(pub Box<dyn PeopleCull>);

/// `cullPeople`'s memory: who has cloth now, and how each person was last
/// told to be drawn.
#[derive(Resource, Debug, Default)]
pub struct PeopleCulling {
    pub clothed: HashSet<Entity>,
    pub last: HashMap<Entity, PersonDetail>,
}

/// `cullPeople`: shows, shadows or hides each person but the player, and
/// sets how much detail they're drawn in.
pub fn cull_people(world: &mut World) {
    if !world.contains_resource::<PeopleCuller>() {
        return;
    }
    let Ok((eye, cull)) = world
        .query::<(&GlobalTransform, &CullEye)>()
        .single(world)
        .map(|(g, c)| (*g, *c))
    else {
        return;
    };
    let v = View::of(&eye, &cull);
    let culler = world
        .remove_resource::<PeopleCuller>()
        .expect("checked above");
    let mut state = world.remove_resource::<PeopleCulling>().unwrap_or_default();
    let people = culler.0.people(world);
    state.clothed = clothed(&v, &people, &state.clothed);
    let mut alive = HashSet::new();
    for p in &people {
        let d = detail(&v, p, state.clothed.contains(&p.body));
        alive.insert(p.body);
        if state.last.get(&p.body) != Some(&d) {
            culler.0.apply(world, p, &d);
            state.last.insert(p.body, d);
        }
    }
    // Forget the people who've gone.
    state.last.retain(|e, _| alive.contains(e));
    world.insert_resource(state);
    world.insert_resource(culler);
}
