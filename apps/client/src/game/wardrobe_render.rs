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

pub fn scale(width: f32, height: f32) -> f32 {
    (height / 760.).min(width / 1150.).clamp(0.7, 1.6)
}
const INK: Color = Color::srgb_u8(46, 48, 43);
const MUTED: Color = Color::srgb_u8(107, 106, 94);
const RULE: Color = Color::srgba_u8(150, 157, 143, 160);
const PAPER: Color = Color::srgb_u8(236, 227, 207);
const HIGHLIGHT: Color = Color::srgba_u8(255, 200, 80, 120);
#[derive(Resource)]
pub struct Fonts {
    typed: Handle<Font>,
    bold: Handle<Font>,
    label: Handle<Font>,
    heading: Handle<Font>,
}
pub fn fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    commands.insert_resource(Fonts {
        typed: fonts.add(Font::from_bytes(
            include_bytes!("../../../../game/fonts/CourierPrime-Regular.ttf").to_vec(),
        )),
        bold: fonts.add(Font::from_bytes(
            include_bytes!("../../../../game/fonts/CourierPrime-Bold.ttf").to_vec(),
        )),
        label: fonts.add(Font::from_bytes(
            include_bytes!("../../../../game/fonts/Inter-SemiBold.ttf").to_vec(),
        )),
        heading: fonts.add(Font::from_bytes(
            include_bytes!("../../../../game/fonts/Inter-Black.ttf").to_vec(),
        )),
    });
}
fn node(x: f32, y: f32, w: f32, h: f32, sc: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(x * sc),
        top: px(y * sc),
        width: px(w * sc),
        height: px(h * sc),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}
fn text(value: impl Into<String>, font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        TextLayout::no_wrap(),
        Pickable::IGNORE,
    )
}
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
            c.spawn((
                node(44., y + 2., 420., h + 8., sc),
                BackgroundColor(Color::srgba_u8(0, 0, 0, 90)),
                Pickable::IGNORE,
            ));
            c.spawn((
                Node {
                    border: UiRect::all(px(sc.max(1.))),
                    ..node(38., y - 4., 420., h + 8., sc)
                },
                BackgroundColor(PAPER),
                BorderColor::all(Color::srgb_u8(204, 192, 166)),
                Pickable::IGNORE,
            ));
            // paper.go's deterministic specks and fibres.
            for i in 0..(420. * (h + 8.) / (14. * 14.)) as i32 {
                let u = earth_two_world::terrain::lattice(i, 1, 0x9a9e);
                let v = earth_two_world::terrain::lattice(i, 2, 0x9a9e);
                let k = earth_two_world::terrain::lattice(i, 3, 0x9a9e);
                let w = if k > 0.8 {
                    2. + 5. * k
                } else {
                    (1. / sc).max(1.)
                };
                let fibre_h = (1. / sc).max(1.);
                c.spawn((
                    node(
                        38. + u * (420. - w),
                        y - 4. + v * (h + 8. - fibre_h),
                        w,
                        fibre_h,
                        sc,
                    ),
                    BackgroundColor(Color::srgba_u8(120, 104, 78, (10. + 22. * k) as u8)),
                    Pickable::IGNORE,
                ));
            }
            for f in [0.22, 0.78] {
                c.spawn((
                    Node {
                        border_radius: BorderRadius::MAX,
                        ..node(47., y - 4. + (h + 8.) * f - 5., 10., 10., sc)
                    },
                    BackgroundColor(Color::srgba_u8(30, 26, 22, 200)),
                    Pickable::IGNORE,
                ));
            }
            c.spawn((
                Node {
                    justify_content: JustifyContent::Start,
                    ..node(76., y + 28., 240., 16., sc)
                },
                Pickable::IGNORE,
            ))
            .with_child(text(
                "THE EXCHANGE  •  LANDFALL",
                &fonts.label,
                11. * sc,
                MUTED,
            ));
            let mut form = node(345., y + 26., 75., 20., sc);
            form.border = UiRect::all(px(sc.max(1.)));
            c.spawn((form, BorderColor::all(INK), Pickable::IGNORE))
                .with_child(text("FORM W-1", &fonts.bold, 11. * sc, INK));
            c.spawn((
                Node {
                    justify_content: JustifyContent::Start,
                    ..node(76., y + 50., 344., 32., sc)
                },
                Pickable::IGNORE,
            ))
            .with_child(text("Wardrobe", &fonts.heading, 24. * sc, INK));
            for dy in [87., 90.] {
                c.spawn((
                    node(76., y + dy, 344., 1., sc),
                    BackgroundColor(INK),
                    Pickable::IGNORE,
                ));
            }
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
            c.spawn((
                node(76., by, 344., 50., sc),
                bevy::ui_widgets::Button,
                bevy::ui_widgets::ActivateOnPress,
                Choice {
                    action: MenuAction::Back,
                    focus: ROW_COUNT,
                },
            ));
            if menu.focus() == ROW_COUNT {
                c.spawn((
                    node(110., by + 10., 310., 30., sc),
                    BackgroundColor(HIGHLIGHT),
                    Pickable::IGNORE,
                ));
            }
            let mut n = node(82., by + 16., 18., 18., sc);
            n.border = UiRect::all(px(sc.max(1.)));
            c.spawn((n, BorderColor::all(INK), Pickable::IGNORE))
                .with_child(text(
                    if menu.focus() == ROW_COUNT { "x" } else { "" },
                    &fonts.bold,
                    18. * sc,
                    Color::srgb_u8(38, 58, 128),
                ));
            c.spawn((
                Node {
                    justify_content: JustifyContent::Start,
                    ..node(116., by, 298., 50., sc)
                },
                Pickable::IGNORE,
            ))
            .with_child(text("Done", &fonts.bold, 18. * sc, INK));
        });
}
#[derive(Component)]
pub struct WardrobeRoot;
