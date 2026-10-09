//! The rows and live preview from game/wardrobe.go.
use super::{Screen, menu::Menu};
use crate::{
    character::{Body, Player},
    presentation::{Outfit, Slot, Wardrobe, outfit::SLOT_COUNT},
};
use bevy::prelude::*;

pub const ROW_BODY: usize = 0;
pub const ROW_TONE: usize = 1;
pub const ROW_LOOK: usize = 2;
pub const ROW_SLOTS: usize = 3;
pub const ROW_COUNT: usize = ROW_SLOTS + SLOT_COUNT;

pub fn cycle_row(w: &Wardrobe, mut o: Outfit, row: usize, step: i32) -> Outfit {
    let Some(body) = w.bodies.get(o.body as usize) else {
        return o;
    };
    let wrap = |i: i32, n: usize| i.rem_euclid(n as i32) as usize;
    match row {
        ROW_BODY if w.bodies.len() > 1 => return w.rebody(&o, wrap(o.body + step, w.bodies.len())),
        ROW_BODY => {}
        ROW_TONE => {
            if !body.tones.is_empty() {
                o.tone = wrap(o.tone + step, body.tones.len()) as i32;
            }
        }
        ROW_LOOK => {
            if !body.looks.is_empty() {
                let next = w
                    .look_of(&o)
                    .map_or(if step < 0 { body.looks.len() - 1 } else { 0 }, |i| {
                        wrap(i as i32 + step, body.looks.len())
                    });
                return w.wear(o, next);
            }
        }
        _ => {
            if let Some(&slot) = row.checked_sub(ROW_SLOTS).and_then(|i| Slot::ALL.get(i)) {
                let n = body.items(slot).len();
                if n > 0 {
                    let current = o.item(slot).map_or(0, |i| i + 1);
                    let next = wrap(current as i32 + step, n + 1);
                    if next == 0 {
                        o.remove(slot);
                    } else {
                        o.put(slot, next - 1);
                    }
                }
            }
        }
    }
    o
}
pub fn row_text(w: &Wardrobe, o: Outfit, row: usize) -> (String, String) {
    let Some(body) = w.bodies.get(o.body as usize) else {
        return (String::new(), String::new());
    };
    let (label, value) = match row {
        ROW_BODY => {
            let mut name = body.name.chars();
            return (
                "Body".into(),
                name.next().map_or_else(String::new, |c| {
                    c.to_uppercase().collect::<String>() + name.as_str()
                }),
            );
        }
        ROW_TONE => (
            "Skin",
            body.tones.get(o.tone as usize).map_or("-", |t| &t.name),
        ),
        ROW_LOOK => ("Look", w.look_of(&o).map_or("Own", |i| &body.looks[i].name)),
        _ => {
            let Some(&slot) = row.checked_sub(ROW_SLOTS).and_then(|i| Slot::ALL.get(i)) else {
                return (String::new(), String::new());
            };
            (
                slot.name(),
                o.item(slot).and_then(|i| body.items(slot).get(i)).map_or(
                    if slot == Slot::Face {
                        "Standard"
                    } else {
                        "None"
                    },
                    |i| &i.name,
                ),
            )
        }
    };
    (label.into(), value.into())
}

pub fn face_camera(
    menu: Res<Menu>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    players: Query<(), With<Player>>,
    mut bodies: Query<(&ChildOf, &mut Transform), With<Body>>,
) {
    if *screen.get() != Screen::Dressing && !menu.on_title() {
        return;
    }
    let k = (8. * time.delta_secs()).min(1.);
    for (parent, mut tr) in &mut bodies {
        if players.contains(parent.parent()) {
            tr.rotation = tr.rotation.slerp(Quat::IDENTITY, k);
        }
    }
}
