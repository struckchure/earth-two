//! Contextual HUD, world clock and testing controls from screens/clock/testing.go.
use super::{
    Screen,
    crowd::TestCrowd,
    menu::Menu,
    paper::*,
    residents::Residents,
    settings::{Settings, WEATHER_NAMES, time_index, weather_index},
};
use crate::{
    character::{LifeState, Player, Traversal},
    identity::IdentityPanel,
    shading::ShadowQuality,
    sky::{
        Clock, Daylight, Weather,
        clock::{DAYLIGHT_HOURS, day_strip, part_of_day},
        weather::conditions,
    },
    vehicle::{Driving, Prompt},
};
use bevy::{prelude::*, window::PrimaryWindow};
const LIGHT: Color = Color::srgb_u8(245, 240, 232);
const MUTED_HUD: Color = Color::srgb_u8(178, 168, 156);
const ACCENT: Color = Color::srgb_u8(255, 184, 82);
const PANEL: Color = Color::srgba_u8(16, 13, 11, 214);
#[derive(Component)]
pub(super) struct HudRoot;
#[derive(Default)]
pub(super) struct FrameRate {
    sum: f32,
    frames: usize,
    pub fps: usize,
}
impl FrameRate {
    fn tick(&mut self, dt: f32) {
        self.sum += dt;
        self.frames += 1;
        if self.sum >= 0.5 {
            self.fps = (self.frames as f32 / self.sum).round() as usize;
            self.sum = 0.;
            self.frames = 0;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn line(
    c: &mut ChildSpawnerCommands,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: &str,
    size: f32,
    ink: Color,
    f: &Fonts,
    sc: f32,
) {
    label(
        c,
        Rect::new(x, y, x + w, y + h),
        s,
        &f.label,
        size.min(w / (s.chars().count().max(1) as f32 * 0.57)),
        ink,
        sc,
    );
}
#[allow(clippy::too_many_arguments)]
fn right_line(
    c: &mut ChildSpawnerCommands,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    value: &str,
    size: f32,
    ink: Color,
    f: &Fonts,
    sc: f32,
) {
    c.spawn((
        Node {
            justify_content: JustifyContent::End,
            ..node(x, y, w, h, sc)
        },
        Pickable::IGNORE,
    ))
    .with_child(text(value, &f.label, size * sc, ink));
}
fn key(c: &mut ChildSpawnerCommands, at: Vec2, k: &str, f: &Fonts, sc: f32) -> f32 {
    let w = (k.len() as f32 * 8. + 14.).max(22.1);
    rect(
        c,
        Rect::from_corners(at, at + Vec2::new(w, 23.8)),
        Color::srgba_u8(255, 255, 255, 80),
        sc,
    );
    rect(
        c,
        Rect::from_corners(at + Vec2::ONE, at + Vec2::new(w - 1., 22.8)),
        Color::srgb_u8(58, 52, 46),
        sc,
    );
    c.spawn((node(at.x, at.y, w, 23.8, sc), Pickable::IGNORE))
        .with_child(text(k, &f.label, 13. * sc, LIGHT));
    w
}
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct HudData<'w> {
    menu: Res<'w, Menu>,
    screen: Res<'w, State<Screen>>,
    f: Res<'w, Fonts>,
    time: Res<'w, Time>,
    day: Res<'w, Daylight>,
    weather: Res<'w, Weather>,
    clock: Res<'w, Clock>,
    shadows: Res<'w, ShadowQuality>,
    settings: Res<'w, Settings>,
    driving: Res<'w, Driving>,
    prompt: Res<'w, Prompt>,
    crowd: Res<'w, TestCrowd>,
    residents: Res<'w, Residents>,
    identity: Res<'w, IdentityPanel>,
}
pub(super) fn draw(
    mut commands: Commands,
    data: HudData,
    window: Query<&Window, With<PrimaryWindow>>,
    player: Query<&Traversal, With<Player>>,
    roots: Query<Entity, With<HudRoot>>,
    mut fps: Local<FrameRate>,
    mut previous: Local<String>,
) {
    let HudData {
        menu,
        screen,
        f,
        time,
        day,
        weather,
        clock,
        shadows,
        settings,
        driving,
        prompt,
        crowd,
        residents,
        identity,
    } = data;
    let Ok(window) = window.single() else {
        return;
    };
    fps.tick(time.delta_secs());
    let size = Vec2::new(window.width(), window.height());
    let sc = scale(size.x, size.y);
    let w = size.x / sc;
    let h = size.y / sc;
    let screen = if *screen.get() == Screen::Loading {
        Screen::Loading
    } else {
        menu.screen()
    };
    let hint = player.single().map_or("", |t| t.hint.as_str());
    let (hh, mm, _) = day.now.clock();
    let hour = day.now.hour_of();
    let (connection, tint) = identity.connection_label();
    let gear = if driving.gear < 0 {
        "R".into()
    } else if driving.gear > 0 {
        driving.gear.to_string()
    } else if driving.speed.abs() < 0.5 {
        "N".into()
    } else {
        String::new()
    };
    let critical = residents
        .list
        .iter()
        .filter(|r| r.health.state == LifeState::Critical)
        .count();
    let dead = residents
        .list
        .iter()
        .filter(|r| r.health.state == LifeState::Dead)
        .count();
    let rows = vec![
        (
            "P",
            "Teleport",
            "to the map's pointer, or the marked spot".into(),
        ),
        (
            "F5",
            "Time of day",
            DAYLIGHT_HOURS[time_index(&clock)].0.into(),
        ),
        (
            "F6",
            "Weather",
            WEATHER_NAMES[weather_index(&weather)].into(),
        ),
        ("F7", "Shadows", format!("{shadows:?}")),
        (
            "F8",
            "NPCs",
            if crowd.enabled { "On" } else { "Off" }.into(),
        ),
        (
            "F9",
            "NPC count",
            if crowd.editing {
                format!("{}_", crowd.digits)
            } else {
                format!(
                    "{} ({} placed, {} near)",
                    crowd.population, crowd.live, residents.near
                )
            },
        ),
        ("", "", format!("Critical: {critical} · Dead: {dead}")),
        (
            "",
            "",
            if crowd.editing {
                "Type 0–4199; Enter saves; Esc cancels"
            } else {
                "F9 edits the count; NPCs spawn nearby"
            }
            .into(),
        ),
    ];
    let state = format!(
        "{screen:?}{size:?}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{:?}{:?}",
        fps.fps,
        connection,
        hh,
        mm,
        conditions(weather.storm),
        hint,
        prompt.key,
        prompt.text,
        prompt.noting(),
        driving.active(),
        (driving.speed.abs() * 3.6).round(),
        gear,
        driving.name,
        driving.headlamps,
        crowd.shown,
        settings.volume,
        rows,
        tint
    );
    if *previous == state {
        return;
    }
    *previous = state;
    for e in &roots {
        commands.entity(e).despawn();
    }
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(20),
        ))
        .with_children(|c| {
            // FPS and connection remain visible over the menus, as in Go.
            let counter = format!("{} FPS", fps.fps);
            let fw = counter.len() as f32 * 7.5 + 18.;
            rect(
                c,
                Rect::new(20., h - 38., 20. + fw, h - 14.),
                Color::srgba_u8(16, 13, 11, 150),
                sc,
            );
            line(
                c,
                29.,
                h - 38.,
                fw - 18.,
                24.,
                &counter,
                13.,
                MUTED_HUD,
                &f,
                sc,
            );
            let x = 28. + fw;
            let cw = connection.chars().count() as f32 * 7.5 + 18.;
            rect(
                c,
                Rect::new(x, h - 38., x + cw, h - 14.),
                Color::srgba_u8(16, 13, 11, 150),
                sc,
            );
            line(
                c,
                x + 9.,
                h - 38.,
                cw - 18.,
                24.,
                &connection,
                13.,
                Color::srgba_u8(tint[0], tint[1], tint[2], tint[3]),
                &f,
                sc,
            );
            if screen == Screen::Playing {
                let mini = crate::landfall::maps::MINIMAP_SIZE;
                let y = 16. + mini + 3. + 6.;
                let cw = mini + 6.;
                rect(c, Rect::new(13., y, 13. + cw, y + 50.), PANEL, sc);
                line(
                    c,
                    23.,
                    y + 4.,
                    cw * 0.48,
                    38.,
                    &format!("{hh:02}:{mm:02}"),
                    24.,
                    LIGHT,
                    &f,
                    sc,
                );
                right_line(
                    c,
                    13. + cw * 0.5,
                    y + 5.,
                    cw * 0.5 - 10.,
                    17.,
                    part_of_day(day.sun, hour),
                    12.,
                    LIGHT,
                    &f,
                    sc,
                );
                let weather_name = conditions(weather.storm);
                let ink = match weather_name {
                    "Dusty" => ACCENT,
                    "Dust storm" => Color::srgb_u8(255, 120, 72),
                    _ => MUTED_HUD,
                };
                right_line(
                    c,
                    13. + cw * 0.5,
                    y + 22.,
                    cw * 0.5 - 10.,
                    17.,
                    weather_name,
                    12.,
                    ink,
                    &f,
                    sc,
                );
                let per = (cw - 20.) / 24.;
                for (i, color) in day_strip().iter().enumerate() {
                    let x = 23. + i as f32 * per;
                    rect(
                        c,
                        Rect::new(x, y + 42., x + per + 0.5, y + 45.),
                        color.to_bevy(),
                        sc,
                    );
                }
                let x = 23. + hour as f32 / 24. * (cw - 20.);
                rect(c, Rect::new(x - 1., y + 39., x + 1., y + 48.), LIGHT, sc);
                if !hint.is_empty() {
                    line(c, 20., h - 120., w - 190., 24., hint, 15., LIGHT, &f, sc);
                }
                let mut py = h - 84.;
                if !prompt.key.is_empty() {
                    let x = 20. + key(c, Vec2::new(20., py), &prompt.key, &f, sc) + 8.;
                    line(
                        c,
                        x,
                        py,
                        w - x - 175.,
                        24.,
                        &prompt.text,
                        15.,
                        LIGHT,
                        &f,
                        sc,
                    );
                    py -= 30.;
                }
                if !prompt.noting().is_empty() {
                    line(
                        c,
                        20.,
                        py,
                        w - 195.,
                        24.,
                        prompt.noting(),
                        15.,
                        ACCENT,
                        &f,
                        sc,
                    );
                }
                if driving.active() {
                    let x = w - 150.;
                    let y = h - 104.;
                    rect(c, Rect::new(x, y, x + 130., y + 84.), PANEL, sc);
                    line(
                        c,
                        x + 14.,
                        y + 8.,
                        76.,
                        46.,
                        &format!("{:.0}", driving.speed.abs() * 3.6),
                        38.,
                        LIGHT,
                        &f,
                        sc,
                    );
                    line(c, x + 86., y + 8., 30., 46., &gear, 22., ACCENT, &f, sc);
                    line(
                        c,
                        x + 14.,
                        y + 54.,
                        102.,
                        20.,
                        &format!("km/h · {}", driving.name),
                        12.,
                        MUTED_HUD,
                        &f,
                        sc,
                    );
                    // Keep the lamp hint clear of the persistent connection status.
                    let x = (28. + fw + cw + 12.).max(130.);
                    let k = key(c, Vec2::new(x, h - 38.), "H", &f, sc);
                    line(
                        c,
                        x + k + 8.,
                        h - 38.,
                        (w - x - k - 180.).max(70.),
                        24.,
                        if driving.headlamps {
                            "Headlamps on"
                        } else {
                            "Headlamps off"
                        },
                        13.,
                        LIGHT,
                        &f,
                        sc,
                    );
                }
            }
            if crowd.shown && matches!(screen, Screen::Playing | Screen::Mapping) {
                let width = 420f32.min(w - 28.);
                let x = w - width - 14.;
                let y = 170.;
                rect(
                    c,
                    Rect::new(x, y, x + width, y + 40. + rows.len() as f32 * 30.),
                    PANEL,
                    sc,
                );
                line(
                    c,
                    x + 14.,
                    y + 10.,
                    width - 28.,
                    24.,
                    "Testing",
                    15.,
                    ACCENT,
                    &f,
                    sc,
                );
                for (i, (k, name, value)) in rows.iter().enumerate() {
                    let y = y + 36. + i as f32 * 30.;
                    if k.is_empty() {
                        line(
                            c,
                            x + 14.,
                            y,
                            width - 28.,
                            24.,
                            value,
                            13.,
                            MUTED_HUD,
                            &f,
                            sc,
                        );
                    } else {
                        key(c, Vec2::new(x + 14., y), k, &f, sc);
                        line(c, x + 58., y, 80., 24., name, 14., MUTED_HUD, &f, sc);
                        line(c, x + 138., y, width - 152., 24., value, 14., LIGHT, &f, sc);
                    }
                }
            }
        });
}
