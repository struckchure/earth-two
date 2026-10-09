//! Shared paper and ink primitives from game/paper.go, in logical points.
use super::render::Choice;
use bevy::prelude::*;

pub fn scale(width: f32, height: f32) -> f32 {
    (height / 760.).min(width / 1150.).clamp(0.7, 1.6)
}
pub(super) const INK: Color = Color::srgb_u8(46, 48, 43);
pub(super) const MUTED: Color = Color::srgb_u8(107, 106, 94);
pub(super) const RULE: Color = Color::srgba_u8(150, 157, 143, 160);
pub(super) const PAPER: Color = Color::srgb_u8(236, 227, 207);
pub(super) const HIGHLIGHT: Color = Color::srgba_u8(255, 200, 80, 120);
#[derive(Resource)]
pub struct Fonts {
    pub(super) typed: Handle<Font>,
    pub(super) bold: Handle<Font>,
    pub(super) label: Handle<Font>,
    pub(super) heading: Handle<Font>,
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
pub(super) fn node(x: f32, y: f32, w: f32, h: f32, sc: f32) -> Node {
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
pub(super) fn text(
    value: impl Into<String>,
    font: &Handle<Font>,
    size: f32,
    color: Color,
) -> impl Bundle {
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

pub fn rect(c: &mut ChildSpawnerCommands, r: Rect, color: Color, sc: f32) {
    c.spawn((
        node(r.min.x, r.min.y, r.width(), r.height(), sc),
        BackgroundColor(color),
        Pickable::IGNORE,
    ));
}
#[allow(clippy::too_many_arguments)]
pub fn label(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    color: Color,
    sc: f32,
) {
    c.spawn((
        Node {
            justify_content: JustifyContent::Start,
            ..node(r.min.x, r.min.y, r.width(), r.height(), sc)
        },
        Pickable::IGNORE,
    ))
    .with_child(text(value, font, size * sc, color));
}
pub fn stroke(c: &mut ChildSpawnerCommands, a: Vec2, b: Vec2, thick: f32, color: Color, sc: f32) {
    let d = b - a;
    let mid = (a + b) * 0.5;
    c.spawn((
        node(
            mid.x - d.length() / 2.,
            mid.y - thick / 2.,
            d.length(),
            thick,
            sc,
        ),
        UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
        BackgroundColor(color),
        Pickable::IGNORE,
    ));
}
pub fn sheet(c: &mut ChildSpawnerCommands, r: Rect, sc: f32) {
    rect(
        c,
        Rect::from_corners(r.min + Vec2::splat(6.), r.max + Vec2::splat(6.)),
        Color::srgba_u8(0, 0, 0, 90),
        sc,
    );
    c.spawn((
        Node {
            border: UiRect::all(px(sc.max(1.))),
            ..node(r.min.x, r.min.y, r.width(), r.height(), sc)
        },
        BackgroundColor(PAPER),
        BorderColor::all(Color::srgb_u8(204, 192, 166)),
        Pickable::IGNORE,
    ));
    for i in 0..(r.width() * r.height() / 196.) as i32 {
        let u = earth_two_world::terrain::lattice(i, 1, 0x9a9e);
        let v = earth_two_world::terrain::lattice(i, 2, 0x9a9e);
        let k = earth_two_world::terrain::lattice(i, 3, 0x9a9e);
        let w = if k > 0.8 {
            2. + 5. * k
        } else {
            (1. / sc).max(1.)
        };
        let h = (1. / sc).max(1.);
        let at = r.min + Vec2::new(u * (r.width() - w), v * (r.height() - h));
        rect(
            c,
            Rect::from_corners(at, at + Vec2::new(w, h)),
            Color::srgba_u8(120, 104, 78, (10. + 22. * k) as u8),
            sc,
        );
    }
    for f in [0.22, 0.78] {
        c.spawn((
            Node {
                border_radius: BorderRadius::MAX,
                ..node(r.min.x + 9., r.min.y + r.height() * f - 5., 10., 10., sc)
            },
            BackgroundColor(Color::srgba_u8(30, 26, 22, 200)),
            Pickable::IGNORE,
        ));
    }
}
#[allow(clippy::too_many_arguments)]
pub fn header(
    c: &mut ChildSpawnerCommands,
    at: Vec2,
    width: f32,
    number: &str,
    title: &str,
    size: f32,
    f: &Fonts,
    sc: f32,
) -> f32 {
    label(
        c,
        Rect::from_corners(at, at + Vec2::new(width * 0.7, 16.)),
        "THE EXCHANGE  •  LANDFALL",
        &f.label,
        11.,
        MUTED,
        sc,
    );
    let w = number.len() as f32 * 6.6 + 14.;
    c.spawn((
        Node {
            border: UiRect::all(px(sc.max(1.))),
            ..node(at.x + width - w, at.y - 2., w, 20., sc)
        },
        BorderColor::all(INK),
        Pickable::IGNORE,
    ))
    .with_child(text(number, &f.bold, 11. * sc, INK));
    let y = at.y + 22.;
    label(
        c,
        Rect::new(at.x, y, at.x + width, y + size * 1.25),
        title,
        &f.heading,
        size,
        INK,
        sc,
    );
    let y = y + size * 1.25 + 6.;
    for dy in [0., 3.] {
        rect(
            c,
            Rect::new(at.x, y + dy, at.x + width, y + dy + (1. / sc).max(1.)),
            INK,
            sc,
        );
    }
    y + 12.
}
#[allow(clippy::too_many_arguments)]
pub fn choice(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    label_text: &str,
    hot: bool,
    choice: Option<Choice>,
    f: &Fonts,
    sc: f32,
) {
    if let Some(choice) = choice {
        c.spawn((
            node(r.min.x, r.min.y, r.width(), r.height(), sc),
            bevy::ui_widgets::Button,
            bevy::ui_widgets::ActivateOnPress,
            choice,
        ));
    }
    if hot {
        rect(
            c,
            Rect::new(
                r.min.x + 34.,
                r.min.y + r.height() * 0.2,
                r.max.x,
                r.min.y + r.height() * 0.8,
            ),
            HIGHLIGHT,
            sc,
        );
    }
    let at = r.min + Vec2::new(6., (r.height() - 18.) / 2.);
    c.spawn((
        Node {
            border: UiRect::all(px((1.5 * sc).max(1.))),
            ..node(at.x, at.y, 18., 18., sc)
        },
        BorderColor::all(INK),
        Pickable::IGNORE,
    ));
    if hot {
        let ink = Color::srgb_u8(38, 58, 128);
        stroke(
            c,
            at + Vec2::new(3., 9.),
            at + Vec2::new(7.5, 14.),
            2.5,
            ink,
            sc,
        );
        stroke(
            c,
            at + Vec2::new(7.5, 14.),
            at + Vec2::new(17., -2.),
            2.5,
            ink,
            sc,
        );
    }
    label(
        c,
        Rect::new(r.min.x + 40., r.min.y, r.max.x - 6., r.max.y),
        label_text,
        &f.bold,
        18.,
        INK,
        sc,
    );
    rect(
        c,
        Rect::new(r.min.x, r.max.y - (1. / sc).max(1.), r.max.x, r.max.y),
        RULE,
        sc,
    );
}
pub fn seal(c: &mut ChildSpawnerCommands, at: Vec2, f: &Fonts, sc: f32) {
    let dark = Color::srgb_u8(110, 80, 32);
    let brass = Color::srgb_u8(176, 136, 62);
    c.spawn((
        Node {
            border_radius: BorderRadius::MAX,
            ..node(at.x - 28.5, at.y - 28., 60., 60., sc)
        },
        BackgroundColor(Color::srgba_u8(0, 0, 0, 70)),
        Pickable::IGNORE,
    ));
    for (radius, color) in [(30., dark), (28.2, brass), (19.8, dark), (18.6, brass)] {
        c.spawn((
            Node {
                border_radius: BorderRadius::MAX,
                ..node(at.x - radius, at.y - radius, radius * 2., radius * 2., sc)
            },
            BackgroundColor(color),
            Pickable::IGNORE,
        ));
    }
    let rim = "REGISTRY OF LANDFALL * EXCHANGE * ";
    for (i, ch) in rim.chars().enumerate() {
        let a = i as f32 / rim.len() as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let p = at + Vec2::new(a.cos(), a.sin()) * 24.;
        c.spawn((
            node(p.x - 2., p.y - 3., 4., 6., sc),
            UiTransform::from_rotation(Rot2::radians(a + std::f32::consts::FRAC_PI_2)),
            Pickable::IGNORE,
        ))
        .with_child(text(ch.to_string(), &f.heading, 5.1 * sc, dark));
    }
    c.spawn((
        node(at.x - 30., at.y - 16.8, 60., 24., sc),
        Pickable::IGNORE,
    ))
    .with_child(text("0", &f.heading, 18.6 * sc, dark));
    c.spawn((
        node(at.x - 18.6, at.y + 4.8, 37.2, 9., sc),
        Pickable::IGNORE,
    ))
    .with_child(text("UNLISTED", &f.heading, 6. * sc, dark));
}
