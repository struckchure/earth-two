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

#[test]
fn injuries_recover_at_the_pads_without_rolling_or_retaining_physics() {
    use earth_two_client::character::{
        Downed, Health, Intent, LifeState, Ragdoll, State as PoseState, Traversal,
    };
    use earth_two_world::terrain::ARRIVAL;
    for state in [LifeState::Critical, LifeState::Dead] {
        let mut app = app_at(StartAt::Buggy);
        action(&mut app, MenuAction::Play);
        tick(&mut app, 90);
        let player = player(&app);
        for seated in [false, true] {
            // Each repetition begins at the authored vehicle and may enter it.
            if seated {
                let car = app
                    .world_mut()
                    .query::<(&Drivable, &Transform)>()
                    .iter(app.world())
                    .find(|(d, _)| d.name == "buggy" && d.driver.is_none())
                    .map(|(_, t)| t.translation)
                    .unwrap();
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation = car + Vec3::new(1.6, 0.9, 0.);
                tick(&mut app, 90);
                press(&mut app, KeyCode::KeyE);
                assert!(app.world().get::<Seated>(player).is_some());
            }
            app.world_mut().entity_mut(player).insert(Health {
                state,
                impact_speed: 12.,
                vehicle: None,
            });
            tick(&mut app, 5);
            assert!(app.world().get::<Downed>(player).is_some());
            assert!(app.world().get::<Ragdoll>(player).is_some());
            assert!(app.world().get::<CharacterController>(player).is_none());
            assert!(app.world().get::<Seated>(player).is_none());
            assert!(!app.world().resource::<Driving>().active());
            assert!(
                app.world_mut()
                    .query::<&Drivable>()
                    .iter(app.world())
                    .all(|d| d.driver != Some(player))
            );
            action(&mut app, MenuAction::Pause);
            let before = app.world().get::<Transform>(player).unwrap().translation;
            press(&mut app, KeyCode::KeyR);
            tick(&mut app, 10);
            assert!(app.world().get::<Downed>(player).is_some());
            assert_eq!(
                before,
                app.world().get::<Transform>(player).unwrap().translation
            );
            action(&mut app, MenuAction::Resume);
            press(&mut app, KeyCode::KeyR);
            tick(&mut app, 5);
            assert_eq!(
                app.world().get::<Health>(player).unwrap().state,
                LifeState::Healthy
            );
            assert!(
                app.world().get::<Downed>(player).is_none()
                    && app.world().get::<Ragdoll>(player).is_none()
            );
            assert_eq!(
                app.world().get::<RigidBody>(player),
                Some(&RigidBody::Kinematic)
            );
            assert_eq!(
                app.world().get::<LinearVelocity>(player).unwrap().0,
                Vec3::ZERO
            );
            let at = app.world().get::<Transform>(player).unwrap().translation;
            assert!((at.x - ARRIVAL.x).abs() < 0.01 && (at.z - ARRIVAL.z).abs() < 0.01);
            assert!(!app.world().get::<Intent>(player).unwrap().roll);
            assert!(!app.world().get::<Traversal>(player).unwrap().active());
            for st in app.world_mut().query::<&PoseState>().iter(app.world()) {
                assert!(!st.downed && !st.hidden);
            }
        }
    }
}

#[test]
fn injury_text_matches_go_for_critical_and_fatal_impacts() {
    use earth_two_client::{
        character::{Health, LifeState},
        game::injuries::injury_text,
    };
    assert_eq!(injury_text(&Health::default()), None);
    assert_eq!(
        injury_text(&Health {
            state: LifeState::Critical,
            impact_speed: 25. / 3.6,
            vehicle: None
        })
        .unwrap(),
        "Critical condition\nVehicle impact · 25 km/h\nR  Emergency recovery at the Pads"
    );
    assert_eq!(
        injury_text(&Health {
            state: LifeState::Dead,
            impact_speed: 50. / 3.6,
            vehicle: None
        })
        .unwrap(),
        "Dead\nVehicle impact · 50 km/h\nR  Respawn at the Pads"
    );
}

#[test]
fn residents_walk_on_landfall_and_population_editor_does_not_pause() {
    use earth_two_client::game::{crowd::TestCrowd, residents::ResidentOf};
    let mut app = app_at(StartAt::Crowd);
    action(&mut app, MenuAction::Play);
    tick(&mut app, 120);
    let initial: Vec<_> = app
        .world_mut()
        .query_filtered::<(Entity, &Transform), With<ResidentOf>>()
        .iter(app.world())
        .map(|(e, t)| (e, t.translation))
        .collect();
    assert_eq!(initial.len(), 25);
    tick(&mut app, 180);
    let walkers = initial
        .iter()
        .filter(|(e, at)| {
            let tr = app.world().get::<Transform>(*e).unwrap();
            tr.translation.distance(*at) > 0.4
                && app.world().get::<CharacterController>(*e).unwrap().grounded
        })
        .count();
    assert!(walkers > 0, "no resident walked on the real Pads collision");
    press(&mut app, KeyCode::F9);
    press(&mut app, KeyCode::Digit0);
    press(&mut app, KeyCode::Escape);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Playing
    );
    assert_eq!(app.world().resource::<TestCrowd>().population, 25);
    action(&mut app, MenuAction::Pause);
    let frozen: Vec<_> = initial
        .iter()
        .map(|(e, _)| (*e, app.world().get::<Transform>(*e).unwrap().translation))
        .collect();
    tick(&mut app, 30);
    for (e, at) in frozen {
        assert_eq!(app.world().get::<Transform>(e).unwrap().translation, at);
    }
    action(&mut app, MenuAction::Resume);
    press(&mut app, KeyCode::F8);
    assert_eq!(
        app.world_mut()
            .query::<&ResidentOf>()
            .iter(app.world())
            .count(),
        0
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
fn hull_review_spawn_stays_on_its_streamed_ground() {
    let mut app = app_at(StartAt::Hull);
    let p = player(&app);
    let start = app.world().get::<Transform>(p).unwrap().translation;
    for (tr, pos) in app.world_mut().query_filtered::<(&Transform, &Position), With<earth_two_client::landfall::terrain::TerrainBody>>().iter(app.world()) {
        assert_eq!(tr.translation, pos.0, "streamed terrain render/physics locations differ");
    }
    action(&mut app, MenuAction::Play);
    tick(&mut app, 180);
    let at = app.world().get::<Transform>(p).unwrap().translation;
    assert!(
        (at.x - start.x).abs() < 2. && (at.z - start.z).abs() < 2.,
        "Hull spawn left {start:?} for {at:?}"
    );
    assert!(at.y > -1., "fell through Hull ground: {at:?}");
}

#[test]
fn wardrobe_keyboard_freezes_world_updates_body_and_preserves_parent() {
    use earth_two_client::{
        game::menu::Menu,
        presentation::{
            Outfit, Slot, Wardrobe,
            outfit::{Garment, load_wardrobe},
        },
    };
    let mut app = app_at(StartAt::Arrival);
    let w = load_wardrobe(
        include_str!("../../../assets/characters/wardrobe.json"),
        &content::people(),
        |_| None,
    )
    .unwrap();
    app.insert_resource(w);
    let p = player(&app);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Dressing
    );
    let before = app.world().get::<Transform>(p).unwrap().translation;
    press(&mut app, KeyCode::ArrowRight);
    assert_eq!(app.world().get::<Outfit>(p).unwrap().body, 1);
    // Skin, then faction look; Enter cycles a row just like Right.
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Space);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    tick(&mut app, 30);
    assert!(app.world().resource::<Time<Physics>>().is_paused());
    assert_eq!(app.world().get::<Transform>(p).unwrap().translation, before);
    assert_eq!(app.world().resource::<Menu>().focus(), 2);
    let outfit = *app.world().get::<Outfit>(p).unwrap();
    let expected = Slot::ALL
        .iter()
        .filter(|s| outfit.item(**s).is_some())
        .count();
    assert_eq!(
        app.world_mut()
            .query::<&Garment>()
            .iter(app.world())
            .count(),
        expected
    );
    assert_eq!(
        app.world().resource::<Wardrobe>().bodies[outfit.body as usize].name,
        "woman"
    );
    press(&mut app, KeyCode::Backspace);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Title
    );
    assert_eq!(app.world().resource::<Menu>().focus(), 1);
    press(&mut app, KeyCode::ArrowUp);
    press(&mut app, KeyCode::Space);
    tick(&mut app, 30);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Playing
    );
    press(&mut app, KeyCode::Escape);
    press(&mut app, KeyCode::ArrowDown);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Dressing
    );
    assert_eq!(*app.world().get::<Outfit>(p).unwrap(), outfit);
    press(&mut app, KeyCode::Escape);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Paused
    );
    press(&mut app, KeyCode::Escape);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Playing
    );
    assert_eq!(app.world().resource::<Session>().player, Some(p));
}

#[test]
fn controls_navigation_preserves_parent_focus_and_pause_then_native_quit_exits() {
    use earth_two_client::game::menu::{Menu, choices};
    let mut app = app_at(StartAt::Buggy);
    let p = player(&app);
    for parent in [Screen::Title, Screen::Paused] {
        if parent == Screen::Paused {
            action(&mut app, MenuAction::Play);
            tick(&mut app, 15);
            action(&mut app, MenuAction::Pause);
        }
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            *app.world().resource::<State<Screen>>().get(),
            Screen::Controls
        );
        let at = app.world().get::<Transform>(p).unwrap().translation;
        tick(&mut app, 30);
        assert!(app.world().resource::<Time<Physics>>().is_paused());
        assert_eq!(app.world().get::<Transform>(p).unwrap().translation, at);
        // Controls has only Back: both directions wrap to it.
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<Menu>().focus(), 0);
        press(&mut app, KeyCode::Space);
        assert_eq!(*app.world().resource::<State<Screen>>().get(), parent);
        assert_eq!(app.world().resource::<Menu>().focus(), 2);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Backspace);
        assert_eq!(*app.world().resource::<State<Screen>>().get(), parent);
        assert_eq!(app.world().resource::<Menu>().focus(), 2);
    }
    assert_eq!(app.world().resource::<Session>().player, Some(p));
    let last = choices(Screen::Paused).len() - 1;
    action(&mut app, MenuAction::Focus(last));
    press(&mut app, KeyCode::Enter);
    assert!(app.should_exit().is_some());
}
