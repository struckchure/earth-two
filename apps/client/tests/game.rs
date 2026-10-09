//! Application integration, using the real Landfall manifests and the same
//! keyboard-to-intent/seat/physics schedules as the graphical application.
#![cfg(not(feature = "viewer"))]
use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{Body, CharacterController, Player},
    game::{
        GamePlugin, MenuAction, Screen, Session, StartAt,
        camera::{GameCamera, Orbit},
        seats::Seated,
    },
    physics::source_assets,
    presentation::{Roster, content, roster::Skin},
    vehicle::{Drivable, Driving},
    world::LayoutRoot,
};
use std::time::Duration;
fn app_at(start: StartAt) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin {
            file_path: source_assets(),
            ..default()
        },
        bevy::scene::ScenePlugin,
        bevy::state::app::StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1. / 60.,
    )))
    .insert_resource(start)
    .add_plugins(GamePlugin)
    .insert_resource(Roster {
        skins: content::people()
            .into_iter()
            .map(|m| Skin::new(&m.path, m.clips, m.scale))
            .collect(),
    })
    .init_resource::<ButtonInput<KeyCode>>();
    app.world_mut().spawn((GameCamera, Transform::default()));
    app.finish();
    app.cleanup();
    for _ in 0..1500 {
        app.update();
        if *app.world().resource::<State<Screen>>().get() == Screen::Title {
            break;
        }
    }
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Title,
        "{:?}",
        app.world().resource::<Session>().error
    );
    app
}
fn action(app: &mut App, action: MenuAction) {
    app.world_mut().write_message(action);
    app.update();
    app.update();
}
fn player(app: &App) -> Entity {
    app.world().resource::<Session>().player.unwrap()
}
fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }
}
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
}
#[test]
fn title_play_pause_menu_preserve_one_session_and_freeze_physics() {
    let mut app = app_at(StartAt::Buggy);
    let player = player(&app);
    let world_count = app
        .world_mut()
        .query_filtered::<Entity, With<LayoutRoot>>()
        .iter(app.world())
        .count();
    for _ in 0..5 {
        action(&mut app, MenuAction::Play);
        tick(&mut app, 20);
        assert!(!app.world().resource::<Time<Physics>>().is_paused());
        action(&mut app, MenuAction::Pause);
        let at = app.world().get::<Transform>(player).unwrap().translation;
        tick(&mut app, 10);
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation,
            at
        );
        action(&mut app, MenuAction::MainMenu);
        assert_eq!(app.world().resource::<Session>().player, Some(player));
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Player>>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<LayoutRoot>>()
                .iter(app.world())
                .count(),
            world_count
        );
    }
}
#[test]
fn keyboard_moves_player_and_vehicle_entry_exit_restore_the_capsule() {
    let mut app = app_at(StartAt::Buggy);
    action(&mut app, MenuAction::Play);
    tick(&mut app, 90);
    let player = player(&app);
    assert!(
        app.world()
            .get::<CharacterController>(player)
            .unwrap()
            .grounded
    );
    press(&mut app, KeyCode::KeyE);
    assert!(
        app.world().get::<Seated>(player).is_some(),
        "{}",
        app.world()
            .resource::<earth_two_client::vehicle::Prompt>()
            .text
    );
    assert!(app.world().get::<CharacterController>(player).is_none());
    assert!(app.world().get::<Collider>(player).is_none());
    let car = app.world().resource::<Driving>().vehicle.unwrap();
    assert_eq!(
        app.world().get::<Drivable>(car).unwrap().driver,
        Some(player)
    );
    let before = app.world().get::<Transform>(car).unwrap().translation;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    tick(&mut app, 120);
    assert!(
        app.world()
            .get::<Transform>(car)
            .unwrap()
            .translation
            .distance(before)
            > 2.
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    tick(&mut app, 180);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    press(&mut app, KeyCode::KeyE);
    assert!(
        !app.world().resource::<Driving>().active(),
        "{}",
        app.world()
            .resource::<earth_two_client::vehicle::Prompt>()
            .noting()
    );
    assert!(app.world().get::<Seated>(player).is_none());
    assert!(app.world().get::<CharacterController>(player).is_some());
    tick(&mut app, 60);
    let before = app.world().get::<Transform>(player).unwrap().translation;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    tick(&mut app, 120);
    assert!(
        app.world()
            .get::<Transform>(player)
            .unwrap()
            .translation
            .distance(before)
            > 0.5
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Body>>()
            .iter(app.world())
            .count(),
        1
    );
}
#[test]
fn orbit_matches_go_mouse_clamps_and_recenter_delay() {
    let mut orbit = Orbit::default();
    orbit.turn(Vec2::new(100., 1000.));
    assert!((orbit.yaw + 0.4).abs() < 1e-5);
    assert!((orbit.pitch - 65f32.to_radians()).abs() < 1e-5);
    let before = orbit.yaw;
    orbit.recenter(std::f32::consts::PI, true, 1., false);
    assert_eq!(orbit.yaw, before);
    orbit.recenter(std::f32::consts::PI, true, 1., false);
    assert!(orbit.yaw.abs() < before.abs());
    assert!(orbit.forward().is_normalized());
}

#[test]
fn integrated_keyboard_vaults_the_authored_hull_crate() {
    use earth_two_client::{character::Traversal, game::TRAVERSAL_ORIGIN};
    let mut app = app_at(StartAt::Traversal);
    action(&mut app, MenuAction::Play);
    tick(&mut app, 90);
    let player = player(&app);
    assert!(
        app.world()
            .get::<CharacterController>(player)
            .unwrap()
            .grounded
    );
    // Start facing the crate, as in Go's block_vaults_the_crate fixture.
    let body = app
        .world_mut()
        .query_filtered::<Entity, With<Body>>()
        .single(app.world())
        .unwrap();
    app.world_mut().get_mut::<Transform>(body).unwrap().rotation =
        Quat::from_rotation_y(std::f32::consts::PI);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    tick(&mut app, 3);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    let mut vaulted = false;
    for _ in 0..45 {
        tick(&mut app, 1);
        if app.world().get::<Traversal>(player).unwrap().active() {
            vaulted = true;
            break;
        }
    }
    assert!(vaulted, "{:?}", app.world().get::<Traversal>(player));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    tick(&mut app, 90);
    let at = app.world().get::<Transform>(player).unwrap().translation - TRAVERSAL_ORIGIN;
    assert!(at.z < 0.7, "vault should cross crate: {at:?}");
    assert!(!app.world().get::<Traversal>(player).unwrap().active());
}
