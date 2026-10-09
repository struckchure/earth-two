//! Arrival, pause and controls forms from game/screens.go and menu.go.
use super::{
    Screen, Session,
    menu::{Menu, choices},
    paper::*,
    render::{Choice, MenuRoot},
    wardrobe_render::WardrobeRoot,
};
use bevy::{prelude::*, window::PrimaryWindow};

pub const BINDINGS: &[(&[&str], &str)] = &[
    (&["WASD", "Arrows"], "Walk"),
    (&["Mouse"], "Look around"),
    (&["M / J"], "Map / debt and contract journal"),
    (&["Shift"], "Run"),
    (&["Space"], "Jump / vault / mantle / wall kick"),
    (&["Ctrl"], "Slide while running"),
    (&["C"], "Crouch (hold)"),
    (&["R"], "Roll"),
    (&["W / S"], "Climb ladder (move toward to attach)"),
    (&["E"], "Interact"),
    (&["H"], "Vehicle headlamps on / off"),
    (&["F"], "Punch"),
    (&["Q"], "Pick up"),
    (&["T / G"], "Talk / dance"),
    (&["Esc"], "Menu"),
];
pub fn panel_right(screen: Screen) -> f32 {
    match screen {
        Screen::Title => 420.,
        Screen::Paused => 388.,
        Screen::Controls => 548.,
        Screen::Dressing => 448.,
        _ => 0.,
    }
}
fn panel(screen: Screen, height: f32, sc: f32) -> Rect {
    let stack = choices(screen).len() as f32 * 60. - 10.;
    let (x, w, h) = match screen {
        Screen::Title => (80., 340., 128. + stack + 78.),
        Screen::Controls => (
            48.,
            500.,
            28. + 76. + BINDINGS.len() as f32 * 33. + 20. + stack + 28.,
        ),
        _ => (48., 340., 28. + 76. + stack + 28.),
    };
    let y = ((height / sc - h) / 2.).max(24.);
    Rect::new(x, y, x + w, y + h)
}
fn form(
    c: &mut ChildSpawnerCommands,
    screen: Screen,
    r: Rect,
    focus: usize,
    interactive: bool,
    f: &Fonts,
    sc: f32,
) {
    let title = screen == Screen::Title;
    let padding = if title {
        Vec2::new(26., 24.)
    } else {
        Vec2::new(10., 4.)
    };
    sheet(c, Rect::from_corners(r.min - padding, r.max + padding), sc);
    let x = r.min.x + if title { 0. } else { 28. };
    let y = r.min.y + if title { 0. } else { 28. };
    let w = r.width() - if title { 0. } else { 56. };
    let (number, name, size) = match screen {
        Screen::Title => ("FORM A-1", "Earth Two", 40.),
        Screen::Controls => ("CARD C-1", "Controls", 24.),
        _ => ("FORM P-2", "Paused", 24.),
    };
    let line = header(c, Vec2::new(x, y), w, number, name, size, f, sc);
    if title {
        label(
            c,
            Rect::new(x, line, x + w, line + 20.),
            "Arrival registration • the Red, 88 AL",
            &f.typed,
            13.,
            MUTED,
            sc,
        );
    }
    let mut by = r.min.y + if title { 128. } else { 104. };
    if screen == Screen::Controls {
        for (keys, does) in BINDINGS {
            let mut kx = x;
            for (j, key) in keys.iter().enumerate() {
                if j > 0 {
                    c.spawn((node(kx, by, 24., 33., sc), Pickable::IGNORE))
                        .with_child(text("or", &f.typed, 13. * sc, MUTED));
                    kx += 24.;
                }
                let width = (key.len() as f32 * 8. + 14.).max(23.8);
                c.spawn((
                    Node {
                        border: UiRect::all(px(sc.max(1.))),
                        ..node(kx, by + 4.6, width, 23.8, sc)
                    },
                    BackgroundColor(Color::srgb_u8(58, 52, 46)),
                    BorderColor::all(Color::srgba_u8(255, 255, 255, 80)),
                    Pickable::IGNORE,
                ))
                .with_child(text(
                    *key,
                    &f.label,
                    14. * sc,
                    Color::srgb_u8(245, 240, 232),
                ));
                kx += width + 4.;
            }
            label(
                c,
                Rect::new(x + 178., by, x + w, by + 33.),
                does,
                &f.typed,
                14f32.min((w - 178.) / (does.chars().count() as f32 * 0.6)),
                INK,
                sc,
            );
            rect(c, Rect::new(x, by + 32., x + w, by + 33.), RULE, sc);
            by += 33.;
        }
        by += 20.;
    }
    for (i, (name, action)) in choices(screen).into_iter().enumerate() {
        choice(
            c,
            Rect::new(x, by, x + w, by + 50.),
            name,
            focus == i,
            interactive.then_some(Choice { action, focus: i }),
            f,
            sc,
        );
        by += 60.;
    }
    if title {
        let last = by - 10.;
        label(
            c,
            Rect::new(x, last + 30., x + w - 80., last + 46.),
            "Licence held",
            &f.label,
            10.,
            MUTED,
            sc,
        );
        label(
            c,
            Rect::new(x, last + 46., x + w - 80., last + 66.),
            "Unlisted • tier 0",
            &f.typed,
            15.,
            INK,
            sc,
        );
        seal(c, Vec2::new(x + w - 34., last + 42.), f, sc);
    }
}
fn smooth(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0., 1.);
    x * x * (3. - 2. * x)
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    mut menu: ResMut<Menu>,
    window: Query<&Window, With<PrimaryWindow>>,
    fonts: Res<Fonts>,
    time: Res<Time>,
    orbit: Res<super::camera::Orbit>,
    roots: Query<
        Entity,
        (
            With<MenuRoot>,
            Without<WardrobeRoot>,
            Without<super::identity_render::IdentityRoot>,
        ),
    >,
    mut previous: Local<Option<(Screen, usize, Vec2, Option<String>, Option<u32>, u32)>>,
) {
    let Ok(window) = window.single() else { return };
    if let Some(age) = menu.admitted.as_mut() {
        *age += time.delta_secs();
        if *age > 1.5 {
            menu.admitted = None;
        }
    }
    let size = Vec2::new(window.width(), window.height());
    let fade = if *screen.get() == Screen::Title {
        (1. - orbit.title_time).clamp(0., 1.)
    } else {
        0.
    };
    let key = (
        *screen.get(),
        menu.focus(),
        size,
        session.error.clone(),
        menu.admitted.map(f32::to_bits),
        fade.to_bits(),
    );
    if previous.as_ref() == Some(&key) {
        return;
    }
    *previous = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    if matches!(
        *screen.get(),
        Screen::Playing | Screen::Dressing | Screen::Identity | Screen::Mapping
    ) && menu.admitted.is_none()
    {
        return;
    }
    let sc = scale(size.x, size.y);
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(10),
        ))
        .with_children(|c| {
            match *screen.get() {
                Screen::Title => {
                    let width = (680. * sc).min(size.x) / sc;
                    // Match Go's non-overlapping physical-pixel strips. Overlap
                    // would alpha-blend twice and leave dark vertical seams.
                    let physical = window.scale_factor();
                    let step = 2. / (sc * physical);
                    for i in 0..(width / step).ceil() as usize {
                        let x = i as f32 * step;
                        let alpha = 230. * (1. - smooth(0.3, 1., x / width));
                        rect(
                            c,
                            Rect::new(x, 0., x + step, size.y / sc),
                            Color::srgba_u8(12, 9, 7, alpha as u8),
                            sc,
                        );
                    }
                }
                Screen::Paused | Screen::Controls => rect(
                    c,
                    Rect::new(0., 0., size.x / sc, size.y / sc),
                    Color::srgba_u8(0, 0, 0, 110),
                    sc,
                ),
                _ => {}
            }
            if matches!(
                *screen.get(),
                Screen::Title | Screen::Paused | Screen::Controls
            ) {
                form(
                    c,
                    *screen.get(),
                    panel(*screen.get(), size.y, sc),
                    menu.focus(),
                    true,
                    &fonts,
                    sc,
                );
            }
            if *screen.get() == Screen::Loading {
                let r = Rect::new(48., 60., 468., 220.);
                sheet(c, r, sc);
                header(
                    c,
                    r.min + Vec2::splat(28.),
                    364.,
                    "FORM A-1",
                    "Loading Earth Two",
                    24.,
                    &fonts,
                    sc,
                );
                if let Some(error) = &session.error {
                    label(
                        c,
                        Rect::new(76., 150., 440., 200.),
                        error,
                        &fonts.typed,
                        12.,
                        INK,
                        sc,
                    );
                }
            }
            if let Some(age) = menu.admitted {
                let mut r = panel(Screen::Title, size.y, sc);
                let k = smooth(0.54, 1.5, age);
                let away = k * k * (size.y / sc - r.min.y + 40.);
                r.min.y += away;
                r.max.y += away;
                form(c, Screen::Title, r, 0, false, &fonts, sc);
                let drop = smooth(0., 0.14, age);
                let zoom = 1. + 0.7 * (1. - drop);
                let at = Vec2::new(r.center().x, r.min.y + r.height() * 0.4);
                let ink = Color::srgba(36. / 255., 62. / 255., 138. / 255., drop * 0.9);
                let w = 310. * zoom;
                let h = 70. * zoom;
                let turn = (-8. + 6. * earth_two_world::terrain::lattice(8, 1, 0x5a3)).to_radians();
                c.spawn((
                    node(at.x - w / 2., at.y - h / 2., w, h, sc),
                    UiTransform::from_rotation(Rot2::radians(turn)),
                    Pickable::IGNORE,
                ))
                .with_children(|c| {
                    for inset in [0., 7.36 * zoom] {
                        c.spawn((
                            Node {
                                border: UiRect::all(px(if inset == 0. {
                                    5.06 * zoom * sc
                                } else {
                                    2.3 * zoom * sc
                                })),
                                ..node(inset, inset, w - inset * 2., h - inset * 2., sc)
                            },
                            BorderColor::all(ink),
                            Pickable::IGNORE,
                        ));
                    }
                    c.spawn((node(0., 0., w, h, sc), Pickable::IGNORE))
                        .with_child(text("ADMITTED", &fonts.heading, 46. * zoom * sc, ink));
                    // Missing flecks of ink, as in paper.go's worn stamp.
                    for i in 0..(w * h / (81. * zoom * zoom)) as i32 {
                        let x = earth_two_world::terrain::lattice(i, 4, 0x57a9) * w;
                        let y = earth_two_world::terrain::lattice(i, 5, 0x57a9) * h;
                        let edge =
                            (1. + 1.5 * earth_two_world::terrain::lattice(i, 6, 0x57a9)) * zoom;
                        rect(
                            c,
                            Rect::new(x, y, x + edge, y + edge),
                            Color::srgba_u8(236, 227, 207, (200. * drop) as u8),
                            sc,
                        );
                    }
                });
            }
            if fade > 0. {
                rect(
                    c,
                    Rect::new(0., 0., size.x / sc, size.y / sc),
                    Color::srgba(0., 0., 0., fade),
                    sc,
                );
            }
        });
}
