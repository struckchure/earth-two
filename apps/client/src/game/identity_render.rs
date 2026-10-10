//! FORM I-1 from game/identity.go: the same fields, two-column choices and state.
use super::{
    MenuAction, Screen,
    menu::Menu,
    paper::*,
    render::{Choice, MenuRoot},
};
use crate::identity::{FIELD_LABELS, FOOTNOTE, IDENTITY_FIELDS, IdentityAction, IdentityPanel};
use bevy::{prelude::*, window::PrimaryWindow};

#[derive(Component)]
pub(super) struct IdentityRoot;

#[derive(PartialEq)]
pub(super) struct FormState {
    screen: Screen,
    focus: usize,
    size: Vec2,
    account: String,
    fields: [String; IDENTITY_FIELDS],
    enabled: [bool; 9],
    message: String,
    failed: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    menu: Res<Menu>,
    panel: Res<IdentityPanel>,
    window: Query<&Window, With<PrimaryWindow>>,
    fonts: Res<Fonts>,
    roots: Query<Entity, With<IdentityRoot>>,
    mut previous: Local<Option<FormState>>,
) {
    let Ok(window) = window.single() else { return };
    let size = Vec2::new(window.width(), window.height());
    // Cache display values only: never keep a second copy of the passphrase.
    let state = FormState {
        screen: *screen.get(),
        focus: menu.focus(),
        size,
        account: panel.account_line(),
        fields: std::array::from_fn(|i| panel.field_text(i)),
        enabled: IdentityAction::ITEMS.map(|(_, action)| panel.enabled(action)),
        message: panel.message.clone(),
        failed: panel.failed,
    };
    if previous.as_ref() == Some(&state) {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if state.screen != Screen::Identity {
        *previous = Some(state);
        return;
    }
    let sc = scale(size.x, size.y).min(size.x / 792.).min(size.y / 792.);
    let x = (size.x / sc - 760.) / 2.;
    let y = (size.y / sc - 760.) / 2.;
    commands
        .spawn((
            IdentityRoot,
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
            rect(
                c,
                Rect::new(0., 0., size.x / sc, size.y / sc),
                Color::srgba_u8(0, 0, 0, 140),
                sc,
            );
            sheet(c, Rect::new(x, y, x + 760., y + 760.), sc);
            header(
                c,
                Vec2::new(x + 28., y + 28.),
                704.,
                "FORM I-1",
                "Your identity",
                28.,
                &fonts,
                sc,
            );
            label(
                c,
                Rect::new(x + 28., y + 126., x + 732., y + 156.),
                &state.account,
                &fonts.typed,
                12.,
                INK,
                sc,
            );
            for (i, name) in FIELD_LABELS.iter().enumerate() {
                let ry = y + 170. + i as f32 * 54.;
                c.spawn((
                    node(x + 28., ry, 704., 48., sc),
                    bevy::ui_widgets::Button,
                    bevy::ui_widgets::ActivateOnPress,
                    Choice {
                        action: MenuAction::Focus(i),
                        focus: i,
                    },
                ));
                if state.focus == i {
                    rect(
                        c,
                        Rect::new(x + 28., ry, x + 732., ry + 48.),
                        Color::srgb_u8(225, 219, 194),
                        sc,
                    );
                }
                label(
                    c,
                    Rect::new(x + 36., ry, x + 724., ry + 14.),
                    name,
                    &fonts.label,
                    10.,
                    MUTED,
                    sc,
                );
                // Fit long paths/names down to the source form’s 9-point minimum.
                let size =
                    (688. / (state.fields[i].chars().count().max(1) as f32 * 0.6)).clamp(9., 15.);
                c.spawn((
                    Node {
                        justify_content: JustifyContent::Start,
                        overflow: Overflow::clip(),
                        ..node(x + 36., ry + 14., 688., 32., sc)
                    },
                    Pickable::IGNORE,
                ))
                .with_child(text(
                    if state.fields[i].is_empty() {
                        "—"
                    } else {
                        &state.fields[i]
                    },
                    &fonts.typed,
                    size * sc,
                    INK,
                ));
                rect(
                    c,
                    Rect::new(x + 36., ry + 47., x + 724., ry + 48.),
                    RULE,
                    sc,
                );
            }
            for (i, (name, action)) in IdentityAction::ITEMS.iter().enumerate() {
                let bx = x + 28. + (i % 2) as f32 * 352.;
                let by = y + 408. + (i / 2) as f32 * 48.;
                let r = Rect::new(bx, by, bx + if i == 8 { 704. } else { 342. }, by + 42.);
                choice(
                    c,
                    r,
                    name,
                    state.focus == IDENTITY_FIELDS + i,
                    Some(Choice {
                        action: if *action == IdentityAction::Back {
                            MenuAction::Back
                        } else {
                            MenuAction::IdentityAct(*action)
                        },
                        focus: IDENTITY_FIELDS + i,
                    }),
                    &fonts,
                    sc,
                );
                if !state.enabled[i] {
                    rect(c, r, Color::srgba_u8(250, 245, 232, 145), sc);
                }
            }
            // Error text can be longer than the paper; wrap instead of spilling into the world.
            c.spawn((
                Node {
                    justify_content: JustifyContent::Start,
                    ..node(x + 28., y + 662., 704., 40., sc)
                },
                Pickable::IGNORE,
            ))
            .with_child((
                Text::new(&state.message),
                TextFont {
                    font: fonts.typed.clone().into(),
                    font_size: px(12. * sc).into(),
                    ..default()
                },
                TextColor(if state.failed {
                    Color::srgb_u8(156, 58, 46)
                } else {
                    MUTED
                }),
                Pickable::IGNORE,
            ));
            label(
                c,
                Rect::new(x + 28., y + 696., x + 732., y + 724.),
                FOOTNOTE,
                &fonts.typed,
                11.,
                MUTED,
                sc,
            );
        });
    *previous = Some(state);
}

/// The centered identity form still shifts the background shot, as Go's layoutFor.
pub(super) fn panel_fraction(size: Vec2) -> f32 {
    let sc = scale(size.x, size.y).min(size.x / 792.).min(size.y / 792.);
    ((size.x + 760. * sc) / (2. * size.x.max(1.))).min(0.6)
}
