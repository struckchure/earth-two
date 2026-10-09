//! Bevy UI version of the W-1 form in game/wardrobe.go and menu.go.
use super::{
    MenuAction, Screen,
    menu::Menu,
    render::{Choice, MenuRoot},
    wardrobe::{ROW_COUNT, row_text},
};
use crate::{
    character::Player,
    presentation::{Outfit, Wardrobe},
};
use bevy::{prelude::*, window::PrimaryWindow};

use super::paper::*;
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    menu: Res<Menu>,
    wardrobe: Res<Wardrobe>,
    players: Query<&Outfit, With<Player>>,
    window: Query<&Window, With<PrimaryWindow>>,
    fonts: Res<Fonts>,
    roots: Query<Entity, With<WardrobeRoot>>,
    mut previous: Local<Option<(Screen, usize, Outfit, Vec2)>>,
) {
    let (Ok(outfit), Ok(window)) = (players.single(), window.single()) else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    let key = (*screen.get(), menu.focus(), *outfit, size);
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    if *screen.get() != Screen::Dressing {
        return;
    }
    let sc = scale(size.x, size.y);
    let row = 46f32.min(((size.y / sc - 48. - 192.) / ROW_COUNT as f32).max(34.));
    let h = 192. + row * ROW_COUNT as f32;
    let y = ((size.y / sc - h) / 2.).max(24.);
    let box_size = 34f32.min(row - 8.);
    commands
        .spawn((
            WardrobeRoot,
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            sheet(c, Rect::new(38., y - 4., 458., y + h + 4.), sc);
            header(
                c,
                Vec2::new(76., y + 28.),
                344.,
                "FORM W-1",
                "Wardrobe",
                24.,
                &fonts,
                sc,
            );
            for i in 0..ROW_COUNT {
                let ry = y + 104. + i as f32 * row;
                let ay = ry + (row - box_size) / 2. - 2.;
                let (label, value) = row_text(&wardrobe, *outfit, i);
                let focused = menu.focus() == i;
                c.spawn((
                    node(66., ry, 364., row - 4., sc),
                    bevy::ui_widgets::Button,
                    bevy::ui_widgets::ActivateOnPress,
                    Choice {
                        action: MenuAction::Focus(i),
                        focus: i,
                    },
                ));
                if focused {
                    c.spawn((
                        node(66., ry + (row - 4.) * 0.18, 364., (row - 4.) * 0.64, sc),
                        BackgroundColor(HIGHLIGHT),
                        Pickable::IGNORE,
                    ));
                }
                c.spawn((
                    Node {
                        justify_content: JustifyContent::Start,
                        ..node(72., ry, 102., row - 4., sc)
                    },
                    Pickable::IGNORE,
                ))
                .with_child(text(
                    label.to_uppercase(),
                    &fonts.label,
                    11. * sc,
                    MUTED,
                ));
                let between_x = 184. + box_size + 6.;
                let right_x = 424. - box_size;
                let available = right_x - 6. - between_x;
                let font_size = 16f32.min(available / (value.chars().count().max(1) as f32 * 0.6));
                c.spawn((
                    node(between_x, ry, available, row - 4., sc),
                    Pickable::IGNORE,
                ))
                .with_child(text(
                    value,
                    if focused { &fonts.bold } else { &fonts.typed },
                    font_size * sc,
                    INK,
                ));
                for (x, step) in [(184., -1), (right_x, 1)] {
                    let mut n = node(x, ay, box_size, box_size, sc);
                    n.border = UiRect::all(px((1.5 * sc).max(1.)));
                    c.spawn((
                        n,
                        BorderColor::all(INK),
                        bevy::ui_widgets::Button,
                        bevy::ui_widgets::ActivateOnPress,
                        Choice {
                            action: MenuAction::CycleRow { row: i, step },
                            focus: i,
                        },
                        ZIndex(1),
                    ));
                    let centre = Vec2::new(x + box_size / 2., ay + box_size / 2.);
                    let h = box_size * 0.2;
                    let tip = centre + Vec2::X * (step as f32 * h / 2.);
                    let thick = (1.5 / sc).max(2.4);
                    for sign in [-1., 1.] {
                        let a = centre + Vec2::new(-step as f32 * h / 2., sign * h);
                        let d = tip - a;
                        let mid = (tip + a) / 2.;
                        c.spawn((
                            node(
                                mid.x - d.length() / 2.,
                                mid.y - thick / 2.,
                                d.length(),
                                thick,
                                sc,
                            ),
                            UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
                            BackgroundColor(INK),
                            Pickable::IGNORE,
                            ZIndex(2),
                        ));
                    }
                    c.spawn((
                        Node {
                            border_radius: BorderRadius::MAX,
                            ..node(tip.x - thick / 2., tip.y - thick / 2., thick, thick, sc)
                        },
                        BackgroundColor(INK),
                        Pickable::IGNORE,
                        ZIndex(2),
                    ));
                }
                c.spawn((
                    node(66., ry + row - 5., 364., 1., sc),
                    BackgroundColor(RULE),
                    Pickable::IGNORE,
                ));
            }
            let by = y + 114. + ROW_COUNT as f32 * row;
            choice(
                c,
                Rect::new(76., by, 420., by + 50.),
                "Done",
                menu.focus() == ROW_COUNT,
                Some(Choice {
                    action: MenuAction::Back,
                    focus: ROW_COUNT,
                }),
                &fonts,
                sc,
            );
        });
}
#[derive(Component)]
pub struct WardrobeRoot;
