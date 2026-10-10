//! Live controls for the existing time/weather/shadow resources and master audio.
use super::{Screen, crowd::TestCrowd, menu::Menu};
use crate::{
    shading::ShadowQuality,
    sky::{Clock, Weather, clock::DAYLIGHT_HOURS},
};
use bevy::prelude::*;
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub volume: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self { volume: 1. }
    }
}
impl Settings {
    pub fn adjust_volume(&mut self, step: i8) {
        self.volume = (self.volume + step as f32 * 0.1).clamp(0., 1.);
    }
}
pub fn next_shadow(q: &mut ShadowQuality) {
    *q = match q {
        ShadowQuality::Full => ShadowQuality::Low,
        ShadowQuality::Low => ShadowQuality::Off,
        ShadowQuality::Off => ShadowQuality::Full,
    };
}
pub fn time_index(c: &Clock) -> usize {
    DAYLIGHT_HOURS
        .iter()
        .position(|(_, h)| (*h - c.hour).abs() < 0.01)
        .unwrap_or(0)
}
pub fn weather_index(w: &Weather) -> usize {
    if w.forced < 0. {
        0
    } else if w.forced == 0. {
        1
    } else if w.forced < 1. {
        2
    } else {
        3
    }
}
pub const WEATHER_NAMES: [&str; 4] = ["As scheduled", "Clear", "Dusty", "Dust storm"];
#[allow(clippy::too_many_arguments)]
pub(super) fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    crowd: Res<TestCrowd>,
    menu: Res<Menu>,
    mut clock: ResMut<Clock>,
    mut weather: ResMut<Weather>,
    mut shadows: ResMut<ShadowQuality>,
) {
    if crowd.editing || menu.screen() == Screen::Identity {
        return;
    }
    if keys.just_pressed(KeyCode::F5) {
        clock.hour = DAYLIGHT_HOURS[(time_index(&clock) + 1) % 5].1;
    }
    if keys.just_pressed(KeyCode::F6) {
        let i = (weather_index(&weather) + 1) % 4;
        weather.hold([-1., 0., 0.45, 1.][i]);
    }
    if keys.just_pressed(KeyCode::F7) {
        next_shadow(&mut shadows);
    }
}
