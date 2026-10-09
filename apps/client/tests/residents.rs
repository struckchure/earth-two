//! Go testing_npcs_test.go and injuries_test.go on real roster/physics hierarchies.
#![cfg(not(feature = "viewer"))]
use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{
        Body, CharacterController, CharacterPlugin, Downed, Health, Intent, LifeState, Player,
        Ragdoll,
    },
    game::{
        Screen,
        crowd::{self, TestCrowd},
        residents::*,
    },
    physics::EarthPhysicsPlugin,
    presentation::{PresentationPlugin, Roster, Skin, content, outfit::Garment},
};
use earth_two_world::terrain::ARRIVAL;
use std::time::Duration;
fn app(crowd_on: bool) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        EarthPhysicsPlugin,
        CharacterPlugin,
        PresentationPlugin::default(),
        ResidentsPlugin,
    ))
    .init_state::<Screen>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
        16_666_667,
    )))
    .insert_resource(Roster {
        skins: vec![Skin::new("person", content::people()[0].clips.clone(), 1.)],
    })
    .init_resource::<ButtonInput<KeyCode>>()
    .insert_resource(TestCrowd {
        enabled: crowd_on,
        population: 19,
        ..default()
    })
    .add_systems(
        Update,
        (crowd::input, crowd::sync).chain().before(ResidentsSet),
    );
    app.world_mut()
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Playing);
    let player = app
        .world_mut()
        .spawn((Player, Transform::from_translation(ARRIVAL + Vec3::Y * 0.9)))
        .id();
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(4000., 1., 4000.),
        Transform::from_translation(ARRIVAL - Vec3::Y * 0.5),
    ));
    app.finish();
    app.cleanup();
    (app, player)
}
fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }
}
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    tick(app, 1);
}
fn actors(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<ResidentOf>>()
        .iter(app.world())
        .collect()
}
fn resident(at: Vec3) -> Resident {
    Resident {
        feet: at,
        home: at,
        target: at,
        left: 100.,
        ..default()
    }
}
fn counts(app: &mut App, want: usize) {
    assert_eq!(actors(app).len(), want);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Body>>()
            .iter(app.world())
            .count(),
        want
    );
    assert_eq!(app.world().resource::<TestCrowd>().live, want);
    assert_eq!(app.world().resource::<Residents>().list.len(), want);
}
#[test]
fn population_editor_matches_go_and_consumes_menu_keys() {
    let (mut app, _) = app(false);
    tick(&mut app, 1);
    press(&mut app, KeyCode::F8);
    assert!(app.world().resource::<TestCrowd>().enabled);
    press(&mut app, KeyCode::F9);
    assert!(app.world().resource::<TestCrowd>().shown);
    for k in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Backspace,
        KeyCode::Numpad7,
        KeyCode::Enter,
    ] {
        press(&mut app, k);
    }
    assert_eq!(app.world().resource::<TestCrowd>().population, 17);
    press(&mut app, KeyCode::F9);
    press(&mut app, KeyCode::Digit0);
    press(&mut app, KeyCode::Escape);
    assert_eq!(app.world().resource::<TestCrowd>().population, 17);
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Playing
    );
    press(&mut app, KeyCode::F9);
    for k in [
        KeyCode::Digit9,
        KeyCode::Digit8,
        KeyCode::Digit7,
        KeyCode::Digit6,
        KeyCode::Enter,
    ] {
        press(&mut app, k);
    }
    assert_eq!(
        app.world().resource::<TestCrowd>().population,
        crowd::MAX_NPCS
    );
    press(&mut app, KeyCode::F9);
    press(&mut app, KeyCode::Digit0);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<TestCrowd>().population, 0);
    assert!(!app.world().resource::<TestCrowd>().editing);
}
#[test]
fn shrinking_toggling_and_relocation_remove_complete_hierarchies() {
    let (mut app, player) = app(true);
    tick(&mut app, 4);
    counts(&mut app, 19);
    let bodies: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<Body>>()
        .iter(app.world())
        .collect();
    // Representative garment subtree, including a loaded scene's descendants.
    #[derive(Component)]
    struct Worn;
    for body in bodies {
        let garment = app.world_mut().spawn((Worn, ChildOf(body))).id();
        app.world_mut().spawn((Worn, ChildOf(garment)));
    }
    app.world_mut().resource_mut::<TestCrowd>().population = 3;
    tick(&mut app, 1);
    counts(&mut app, 3);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Worn>>()
            .iter(app.world())
            .count(),
        6
    );
    app.world_mut().resource_mut::<TestCrowd>().enabled = false;
    tick(&mut app, 1);
    counts(&mut app, 0);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Worn>>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&Garment>()
            .iter(app.world())
            .count(),
        0
    );
    {
        let mut n = app.world_mut().resource_mut::<TestCrowd>();
        n.enabled = true;
        n.population = 5;
    }
    tick(&mut app, 3);
    counts(&mut app, 5);
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .x += 1000.;
    tick(&mut app, 1);
    counts(&mut app, 0);
    tick(&mut app, 3);
    counts(&mut app, 5);
    assert!(
        app.world()
            .resource::<Residents>()
            .list
            .iter()
            .all(|r| r.home.x > ARRIVAL.x + 980.)
    );
}
#[test]
fn distant_residents_move_as_data_and_reappear_where_they_reached() {
    let (mut app, player) = app(false);
    app.world_mut().resource_mut::<Residents>().list = vec![
        resident(ARRIVAL + Vec3::X * 5.),
        Resident {
            target: ARRIVAL + Vec3::X * 301.,
            ..resident(ARRIVAL + Vec3::X * 300.)
        },
    ];
    tick(&mut app, 2);
    assert_eq!(actors(&mut app).len(), 1);
    tick(&mut app, 120);
    assert!(app.world().resource::<Residents>().list[1].feet.x > ARRIVAL.x + 300.5);
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .x += 300.;
    tick(&mut app, 2);
    let actors = actors(&mut app);
    assert_eq!(actors.len(), 1);
    assert_eq!(app.world().get::<ResidentOf>(actors[0]).unwrap().index, 1);
    assert!(
        app.world()
            .get::<Transform>(actors[0])
            .unwrap()
            .translation
            .x
            > ARRIVAL.x + 300.5
    );
}
#[test]
fn streamed_injured_residents_keep_health_and_return_prone() {
    for state in [LifeState::Critical, LifeState::Dead] {
        let (mut app, player) = app(false);
        app.world_mut()
            .resource_mut::<Residents>()
            .list
            .push(resident(ARRIVAL + Vec3::X * 5.));
        tick(&mut app, 3);
        let actor = actors(&mut app)[0];
        app.world_mut().entity_mut(actor).insert(Health {
            state,
            impact_speed: 12.,
            vehicle: None,
        });
        tick(&mut app, 30);
        assert_eq!(
            app.world().resource::<Residents>().list[0].health.state,
            state
        );
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x += 300.;
        tick(&mut app, 2);
        assert!(app.world().get_entity(actor).is_err());
        let feet = app.world().resource::<Residents>().list[0].feet;
        tick(&mut app, 60);
        assert_eq!(app.world().resource::<Residents>().list[0].feet, feet);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x -= 300.;
        tick(&mut app, 2);
        let actor = actors(&mut app)[0];
        assert_eq!(app.world().get::<Health>(actor).unwrap().state, state);
        assert!(app.world().get::<CharacterController>(actor).is_none());
        assert!(
            app.world().get::<Downed>(actor).is_some()
                && app.world().get::<Ragdoll>(actor).is_some()
        );
        assert!(
            (app.world().get::<Transform>(actor).unwrap().rotation * Vec3::Y)
                .y
                .abs()
                < 0.2
        );
    }
}
#[test]
fn resident_batches_limits_hysteresis_and_generations_match_go() {
    let (mut app, player) = app(false);
    // Keep physics disabled here: this test exercises resident selection, not overlapping capsules.
    earth_two_client::physics::set_paused(
        &mut app.world_mut().resource_mut::<Time<Physics>>(),
        true,
    );
    app.world_mut().resource_mut::<Residents>().list = (0..450)
        .map(|i| resident(ARRIVAL + Vec3::X * (5. + i as f32 * 0.02)))
        .collect();
    tick(&mut app, 1);
    assert_eq!(actors(&mut app).len(), EMBODY_BATCH);
    tick(&mut app, 30);
    assert_eq!(actors(&mut app).len(), MAX_EMBODIED);
    // New nearer people can retain up to the Go slack of existing actors.
    app.world_mut()
        .resource_mut::<Residents>()
        .list
        .extend((0..30).map(|i| resident(ARRIVAL + Vec3::X * (0.1 + i as f32 * 0.01))));
    tick(&mut app, 2);
    assert_eq!(actors(&mut app).len(), MAX_EMBODIED + EMBODY_SLACK);
    // Inside release radius but outside embody radius, existing characters remain.
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .x -= 48.;
    tick(&mut app, 2);
    assert_eq!(actors(&mut app).len(), MAX_EMBODIED + EMBODY_SLACK);
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation
        .x -= 19.;
    tick(&mut app, 1);
    assert_eq!(actors(&mut app).len(), 0);
    let generation = app.world().resource::<Residents>().generation;
    {
        let mut rs = app.world_mut().resource_mut::<Residents>();
        rs.clear();
        rs.list.push(resident(ARRIVAL - Vec3::X * 45.));
    }
    tick(&mut app, 1);
    let actors = actors(&mut app);
    assert_eq!(actors.len(), 1);
    assert_eq!(
        app.world().get::<ResidentOf>(actors[0]).unwrap().generation,
        generation + 1
    );
}
#[test]
fn resident_intents_stop_in_menus_and_data_freezes() {
    let (mut app, _) = app(false);
    app.world_mut().resource_mut::<Residents>().list = vec![
        Resident {
            left: 0.,
            ..resident(ARRIVAL + Vec3::X * 5.)
        },
        Resident {
            target: ARRIVAL + Vec3::X * 301.,
            ..resident(ARRIVAL + Vec3::X * 300.)
        },
    ];
    tick(&mut app, 4);
    let actor = actors(&mut app)[0];
    assert_ne!(
        app.world().get::<Intent>(actor).unwrap().move_dir,
        Vec3::ZERO
    );
    app.world_mut()
        .resource_mut::<NextState<Screen>>()
        .set(Screen::Paused);
    tick(&mut app, 1);
    let feet = app.world().resource::<Residents>().list[1].feet;
    tick(&mut app, 20);
    assert_eq!(
        app.world().get::<Intent>(actor).unwrap().move_dir,
        Vec3::ZERO
    );
    assert_eq!(app.world().resource::<Residents>().list[1].feet, feet);
}
