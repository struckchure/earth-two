//! game/injuries.go: recovery consumes R before character input and moves
//! the camera with the player. Review routes are opt-in diagnostics only.
use super::{
    Screen, StartAt,
    camera::{GameCamera, Orbit},
};
use crate::{
    character::{self, Downed, Health, LifeState, Player},
    vehicle::{Controls, Drivable, Driving, VehicleInput},
};
use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::terrain::ARRIVAL;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn recover(
    mut commands: Commands,
    keys: Option<ResMut<ButtonInput<KeyCode>>>,
    screen: Res<State<Screen>>,
    physics: Res<Time<Physics>>,
    players: Query<(Entity, &Health, &Transform), With<Player>>,
    mut cameras: Query<&mut Transform, (With<GameCamera>, Without<Player>)>,
    mut orbit: ResMut<Orbit>,
) {
    let Some(mut keys) = keys else { return };
    if *screen.get() != Screen::Playing || physics.is_paused() || !keys.just_pressed(KeyCode::KeyR)
    {
        return;
    }
    let Ok((e, health, at)) = players.single() else {
        return;
    };
    if health.state == LifeState::Healthy {
        return;
    }
    let feet = ARRIVAL + Vec3::Y * 0.05;
    let delta = feet + Vec3::Y * 0.9 - at.translation;
    for mut camera in &mut cameras {
        camera.translation += delta;
    }
    if let Some(target) = &mut orbit.target {
        *target += delta;
    }
    character::revive(&mut commands, e, feet);
    keys.reset_all(); // Recovery must not also request a roll.
}

/// Go's drive.go drops drivers which lost their Seated component. Keep the
/// message-based Rust vehicle tests independent of the application seat bridge.
pub fn release_downed_drivers(
    mut cars: Query<(Entity, &mut Drivable, &mut Controls, &mut VehicleInput)>,
    actors: Query<(Has<Downed>, Has<super::seats::Seated>), With<character::Character>>,
    mut driving: ResMut<Driving>,
) {
    for (e, mut car, mut controls, mut input) in &mut cars {
        if car.driver.is_some_and(|driver| {
            actors
                .get(driver)
                .is_ok_and(|(downed, seated)| downed || !seated)
        }) {
            car.driver = None;
            *controls = default();
            *input = VehicleInput {
                hand_brake: 1.,
                ..default()
            };
            if driving.vehicle == Some(e) {
                *driving = default();
            }
        }
    }
}

/// Stable Go UI text, shared with headless recovery tests.
pub fn injury_text(health: &Health) -> Option<String> {
    let (title, action) = match health.state {
        LifeState::Healthy => return None,
        LifeState::Critical => ("Critical condition", "Emergency recovery at the Pads"),
        LifeState::Dead => ("Dead", "Respawn at the Pads"),
    };
    Some(format!(
        "{title}\nVehicle impact · {:.0} km/h\nR  {action}",
        health.impact_speed * 3.6
    ))
}

/// Reproducible injury/recovery review without adding cheats to normal input.
/// Applies the same knockdown used by vehicle hits; collision thresholds are
/// separately exercised by the vehicle integration suite.
#[allow(clippy::too_many_arguments)]
pub fn review_injury(
    mut commands: Commands,
    start: Res<StartAt>,
    screen: Res<State<Screen>>,
    physics: Res<Time<Physics>>,
    clock: Res<Time>,
    players: Query<(Entity, &Transform), With<Player>>,
    mut elapsed: Local<f32>,
    mut done: Local<bool>,
) {
    let speed = match *start {
        StartAt::Injury => 25. / 3.6,
        StartAt::Fatal => 50. / 3.6,
        _ => return,
    };
    if *done || *screen.get() != Screen::Playing || physics.is_paused() {
        return;
    }
    *elapsed += clock.delta_secs();
    if *elapsed < 2. {
        return;
    }
    let Ok((e, at)) = players.single() else {
        return;
    };
    let health = Health {
        state: crate::vehicle::ImpactRules::default().condition(speed),
        impact_speed: speed,
        vehicle: None,
    };
    character::knock_down(
        &mut commands.entity(e),
        *at,
        health,
        Vec3::new(0., (speed * 0.15).min(3.), speed * 0.6),
    );
    *done = true;
}
