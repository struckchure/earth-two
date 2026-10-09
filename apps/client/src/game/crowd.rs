//! game/testing_npcs.go: opt-in local crowd, not persistent world population.
use super::{
    Screen, StartAt,
    residents::{Resident, Residents, flat_distance},
};
use crate::{
    character::{CharacterPhysics, Player},
    presentation::{Outfit, Roster, Slot, Wardrobe},
};
use bevy::prelude::*;
pub const MAX_NPCS: usize = 4199;
pub const NPC_BATCH: usize = 256;
#[derive(Resource)]
pub struct TestCrowd {
    pub enabled: bool,
    pub population: usize,
    pub live: usize,
    pub editing: bool,
    pub digits: String,
    pub shown: bool,
    pub anchor: Vec3,
    pub site: usize,
}
impl Default for TestCrowd {
    fn default() -> Self {
        Self {
            enabled: false,
            population: 25,
            live: 0,
            editing: false,
            digits: String::new(),
            shown: false,
            anchor: Vec3::ZERO,
            site: 0,
        }
    }
}
pub fn review(start: Res<StartAt>, mut crowd: ResMut<TestCrowd>) {
    if *start == StartAt::Crowd {
        crowd.enabled = true;
        crowd.shown = true;
    }
}
/// Consume the population editor's keys before menu and character input.
pub fn input(keys: Option<ResMut<ButtonInput<KeyCode>>>, mut crowd: ResMut<TestCrowd>) {
    let Some(mut keys) = keys else {
        return;
    };
    if keys.just_pressed(KeyCode::F9) {
        crowd.editing = true;
        crowd.digits.clear();
        crowd.shown = true;
        keys.reset_all();
        return;
    }
    if crowd.editing {
        if keys.just_pressed(KeyCode::F1) {
            crowd.editing = false;
            crowd.shown = false;
        } else if keys.just_pressed(KeyCode::Escape) {
            crowd.editing = false;
        } else if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            if !crowd.digits.is_empty() {
                crowd.population = crowd.digits.parse::<usize>().unwrap_or(0).min(MAX_NPCS);
            }
            crowd.editing = false;
        } else {
            if keys.just_pressed(KeyCode::Backspace) {
                crowd.digits.pop();
            }
            for (digit, pair) in [
                [KeyCode::Digit0, KeyCode::Numpad0],
                [KeyCode::Digit1, KeyCode::Numpad1],
                [KeyCode::Digit2, KeyCode::Numpad2],
                [KeyCode::Digit3, KeyCode::Numpad3],
                [KeyCode::Digit4, KeyCode::Numpad4],
                [KeyCode::Digit5, KeyCode::Numpad5],
                [KeyCode::Digit6, KeyCode::Numpad6],
                [KeyCode::Digit7, KeyCode::Numpad7],
                [KeyCode::Digit8, KeyCode::Numpad8],
                [KeyCode::Digit9, KeyCode::Numpad9],
            ]
            .iter()
            .enumerate()
            {
                if crowd.digits.len() < 5 && keys.any_just_pressed(*pair) {
                    crowd.digits.push(char::from(b'0' + digit as u8));
                }
            }
        }
        keys.reset_all();
        return;
    }
    if keys.just_pressed(KeyCode::F1) {
        crowd.shown = !crowd.shown;
    }
    if keys.just_pressed(KeyCode::F8) {
        crowd.enabled = !crowd.enabled;
    }
}
#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut crowd: ResMut<TestCrowd>,
    mut residents: ResMut<Residents>,
    roster: Res<Roster>,
    wardrobe: Res<Wardrobe>,
    players: Query<(Entity, &Transform), With<Player>>,
    physics: CharacterPhysics,
    screen: Res<State<Screen>>,
) {
    let n = &mut *crowd;
    let mut want = if n.enabled {
        n.population.min(MAX_NPCS)
    } else {
        0
    };
    let player = players.single().ok();
    if player.is_some_and(|(_, t)| n.live > 0 && flat_distance(t.translation, n.anchor) > 128.) {
        want = 0;
    }
    if want == 0 {
        if n.live > 0 {
            residents.clear();
        }
        n.live = 0;
        n.site = 0;
        return;
    }
    if n.live > want {
        residents.truncate(want);
        n.live = want;
    }
    let Some((entity, player)) = player else {
        return;
    };
    if *screen.get() != Screen::Playing || roster.skins.is_empty() {
        return;
    }
    if n.live == 0 {
        n.anchor = player.translation;
        n.site = 0;
    }
    let mut added = 0;
    for _ in 0..NPC_BATCH * 16 {
        if n.live >= want || added >= NPC_BATCH {
            break;
        }
        let at = site(n.anchor, n.site);
        n.site += 1;
        if flat_distance(at, n.anchor) > 120. {
            break;
        }
        let mut feet = Vec3::new(
            at.x,
            earth_two_world::terrain::walk_height(at.x, at.z),
            at.z,
        );
        if let Some(hit) = physics.cast_ray_excluding(
            Vec3::new(at.x, player.translation.y + 1., at.z),
            Vec3::NEG_Y,
            5.,
            entity,
        ) && hit.normal.y > 0.7
            && hit.point.y > feet.y
        {
            feet.y = hit.point.y;
        }
        if physics.overlap_capsule_excluding(feet + Vec3::Y * 0.95, 0.35, 1.8, entity) {
            continue;
        }
        let body = n.live % roster.skins.len();
        residents.list.push(Resident {
            feet,
            skin: body,
            outfit: outfit(&wardrobe, body, n.live),
            home: feet,
            target: feet,
            left: (n.live % 7) as f32,
            ..default()
        });
        n.live += 1;
        added += 1;
    }
}
pub fn site(anchor: Vec3, index: usize) -> Vec3 {
    let angle = index as f64 * 2.399_963_229_728_653;
    let radius = 8. + 1.4 * (index as f64).sqrt();
    anchor
        + Vec3::new(
            (radius * angle.cos()) as f32,
            0.,
            (radius * angle.sin()) as f32,
        )
}
pub fn outfit(wardrobe: &Wardrobe, body: usize, index: usize) -> Outfit {
    let mut outfit = Outfit::new(body as i32, 0);
    let Some(w) = wardrobe.bodies.get(body) else {
        return outfit;
    };
    if !w.tones.is_empty() {
        outfit.tone = (index % w.tones.len()) as i32;
    }
    for slot in [Slot::Hair, Slot::Top, Slot::Bottom, Slot::Shoes] {
        if !w.items(slot).is_empty() {
            outfit.put(slot, index % w.items(slot).len());
        }
    }
    outfit
}
