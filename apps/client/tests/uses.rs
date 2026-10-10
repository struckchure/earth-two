//! World interaction and live settings acceptance through the Landfall schedule.
#![cfg(not(feature = "viewer"))]
use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{Anim, CharacterController, Controls, Intent},
    game::{
        GamePlugin, MenuAction, Screen, Session, StartAt,
        camera::GameCamera,
        contracts::Contracts,
        menu::Menu,
        seats::Seated,
        settings::Settings,
        uses::{FurnitureSeat, Kind, Spot, Uses},
    },
    physics::source_assets,
    presentation::{MotionSamples, Roster, content, roster::Skin},
    shading::ShadowQuality,
    sky::{Clock, Weather},
    vehicle::Prompt,
};
use earth_two_world::kit::{Kit, Layout};
use std::time::Duration;
fn app() -> App {
    let mut a = App::new();
    a.add_plugins((
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
    .insert_resource(StartAt::Arrival)
    .add_plugins(GamePlugin)
    .insert_resource(Roster {
        skins: content::people()
            .into_iter()
            .map(|m| Skin::new(&m.path, m.clips, m.scale))
            .collect(),
    });
    a.world_mut().spawn((GameCamera, Transform::default()));
    a.finish();
    a.cleanup();
    for _ in 0..1500 {
        a.update();
        if screen(&a) == Screen::Title {
            break;
        }
    }
    assert_eq!(
        screen(&a),
        Screen::Title,
        "{:?}",
        a.world().resource::<Session>().error
    );
    assert!(a.world().contains_resource::<Contracts>());
    action(&mut a, MenuAction::Play);
    a
}
fn screen(a: &App) -> Screen {
    a.world().resource::<Menu>().screen()
}
fn tick(a: &mut App, n: usize) {
    for _ in 0..n {
        a.update();
        a.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    }
}
fn action(a: &mut App, act: MenuAction) {
    a.world_mut().write_message(act);
    tick(a, 2);
}
fn press(a: &mut App, key: KeyCode) {
    a.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    a.update();
    a.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    a.update();
}
fn move_to(a: &mut App, feet: Vec3) {
    let player = a.world().resource::<Session>().player.unwrap();
    let height = a.world().get::<CharacterController>(player).unwrap().height;
    let at = feet + Vec3::Y * (height / 2. + 0.1);
    a.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = at;
    if let Some(mut p) = a.world_mut().get_mut::<Position>(player) {
        p.0 = at;
    }
    a.world_mut()
        .get_mut::<CharacterController>(player)
        .unwrap()
        .velocity = Vec3::ZERO;
    a.world_mut()
        .entity_mut(player)
        .insert(MotionSamples::at(at, height));
    tick(a, 100);
}

fn spot(a: &App, kind: Kind) -> Spot {
    a.world().resource::<Uses>().review(kind).unwrap().clone()
}
#[test]
fn authored_use_spots_preserve_placement_rotation_reach_and_floor_limits() {
    let root = source_assets();
    let kit = Kit::load(&root, "world/world.json").unwrap();
    let layout = Layout::load(&root, "world/landfall.json").unwrap();
    let uses = Uses::new(&kit, &layout.pieces);
    for kind in [Kind::Sit, Kind::Repair, Kind::Ring] {
        assert!(uses.cells.values().flatten().any(|s| s.kind == kind));
    }
    let mut kinds = std::collections::HashSet::new();
    for s in uses.cells.values().flatten() {
        assert!(kit.pieces.contains_key(&s.piece));
        kinds.insert(s.piece.clone());
    }
    assert_eq!(kinds.len(), 17);
    let chair = Spot {
        piece: "stool".into(),
        kind: Kind::Sit,
        at: Vec3::new(-8., 0., -8.),
        facing: std::f32::consts::FRAC_PI_2,
    };
    let mut uses = Uses::default();
    uses.cells.insert((-1, -1), vec![chair.clone()]);
    assert!(uses.near(Vec3::new(-7.5, 0., -8.)).is_some());
    assert!(uses.near(Vec3::new(-7.5, 3.6, -8.)).is_none());
    assert!(uses.near(Vec3::new(-4., 0., -8.)).is_none());
    assert!((chair.stand_up().x + 7.45).abs() < 0.001);
    assert!(uses.near(chair.approach() + Vec3::Y * 0.61).is_none());
    assert!(uses.near(chair.approach() - Vec3::Y * 0.91).is_none());
}
#[test]
fn furniture_sit_pause_get_up_and_move_cancel_restore_controller() {
    let mut a = app();
    let s = spot(&a, Kind::Sit);
    move_to(&mut a, s.approach());
    assert_eq!(a.world().resource::<Prompt>().text, "Sit down");
    let p = a.world().resource::<Session>().player.unwrap();
    press(&mut a, KeyCode::KeyE);
    assert!(a.world().get::<FurnitureSeat>(p).is_some());
    assert!(a.world().get::<CharacterController>(p).is_none());
    assert_eq!(a.world().get::<Seated>(p).unwrap().anim, Anim::SitDown);
    let at = a.world().get::<Transform>(p).unwrap().translation;
    action(&mut a, MenuAction::Pause);
    let since = a.world().get::<FurnitureSeat>(p).unwrap().since;
    tick(&mut a, 120);
    assert_eq!(a.world().get::<FurnitureSeat>(p).unwrap().since, since);
    assert_eq!(a.world().get::<Transform>(p).unwrap().translation, at);
    action(&mut a, MenuAction::Resume);
    tick(&mut a, 90);
    assert_eq!(a.world().get::<Seated>(p).unwrap().anim, Anim::Sitting);
    assert_eq!(a.world().resource::<Prompt>().text, "Get up");
    press(&mut a, KeyCode::KeyE);
    assert_eq!(a.world().get::<Seated>(p).unwrap().anim, Anim::SitUp);
    tick(&mut a, 75);
    assert!(a.world().get::<FurnitureSeat>(p).is_none());
    assert!(a.world().get::<Seated>(p).is_none());
    let cc = a.world().get::<CharacterController>(p).unwrap();
    let feet = a.world().get::<Transform>(p).unwrap().translation - Vec3::Y * cc.height / 2.;
    assert!(feet.xz().distance(s.stand_up().xz()) < 0.1);
    assert_eq!(
        a.world().get::<Transform>(p).unwrap().rotation,
        Quat::IDENTITY
    );
    press(&mut a, KeyCode::KeyE);
    tick(&mut a, 90);
    press(&mut a, KeyCode::KeyW);
    tick(&mut a, 75);
    assert!(a.world().get::<CharacterController>(p).is_some());
}
#[test]
fn machine_work_faces_piece_and_movement_stops_it_bell_emits_once() {
    let mut a = app();
    let s = spot(&a, Kind::Repair);
    move_to(&mut a, s.approach());
    assert_eq!(
        a.world().resource::<Prompt>().text,
        "Work on it",
        "spot {s:?}; player {:?}",
        a.world()
            .get::<Transform>(a.world().resource::<Session>().player.unwrap())
    );
    let p = a.world().resource::<Session>().player.unwrap();
    press(&mut a, KeyCode::KeyE);
    assert_eq!(a.world().get::<Intent>(p).unwrap().hold, Anim::Fix);
    assert_eq!(a.world().resource::<Prompt>().text, "Stop working");
    press(&mut a, KeyCode::KeyW);
    assert_eq!(a.world().get::<Intent>(p).unwrap().hold, Anim::Idle);
    let b = spot(&a, Kind::Ring);
    move_to(&mut a, b.approach());
    assert_eq!(a.world().resource::<Prompt>().text, "Ring the bell");
    let mut cursor = a
        .world()
        .resource::<Messages<earth_two_client::game::cues::Cue>>()
        .get_cursor_current();
    press(&mut a, KeyCode::KeyE);
    let bells = cursor
        .read(
            a.world()
                .resource::<Messages<earth_two_client::game::cues::Cue>>(),
        )
        .filter(|c| c.name == "bell")
        .count();
    assert_eq!(bells, 1);
    tick(&mut a, 5);
    assert_eq!(
        cursor
            .read(
                a.world()
                    .resource::<Messages<earth_two_client::game::cues::Cue>>()
            )
            .filter(|c| c.name == "bell")
            .count(),
        0
    );
}
#[test]
fn settings_follow_real_keys_pause_preserve_values_and_bound_master_volume() {
    let mut a = app();
    let original = a.world().resource::<Clock>().hour;
    press(&mut a, KeyCode::F5);
    assert_ne!(a.world().resource::<Clock>().hour, original);
    press(&mut a, KeyCode::F6);
    assert_eq!(a.world().resource::<Weather>().forced, 0.);
    for expected in [ShadowQuality::Low, ShadowQuality::Off, ShadowQuality::Full] {
        press(&mut a, KeyCode::F7);
        assert_eq!(*a.world().resource::<ShadowQuality>(), expected);
    }
    action(&mut a, MenuAction::Pause);
    action(&mut a, MenuAction::Settings);
    assert_eq!(screen(&a), Screen::Settings);
    assert!(!a.world().resource::<Controls>().enabled);
    for _ in 0..15 {
        press(&mut a, KeyCode::Enter);
    }
    assert_eq!(a.world().resource::<Settings>().volume, 0.);
    press(&mut a, KeyCode::ArrowDown);
    for _ in 0..15 {
        press(&mut a, KeyCode::Enter);
    }
    assert_eq!(a.world().resource::<Settings>().volume, 1.);
    press(&mut a, KeyCode::ArrowDown);
    press(&mut a, KeyCode::Enter);
    assert_eq!(*a.world().resource::<ShadowQuality>(), ShadowQuality::Low);
    press(&mut a, KeyCode::Escape);
    assert_eq!(screen(&a), Screen::Paused);
    press(&mut a, KeyCode::Escape);
    assert_eq!(screen(&a), Screen::Playing);
    assert_eq!(*a.world().resource::<ShadowQuality>(), ShadowQuality::Low);
    press(&mut a, KeyCode::F9);
    let held = a.world().resource::<Clock>().hour;
    press(&mut a, KeyCode::F5);
    assert_eq!(a.world().resource::<Clock>().hour, held);
    press(&mut a, KeyCode::Escape);
    assert_eq!(screen(&a), Screen::Playing);
}

#[test]
fn interactions_respect_air_traversal_contract_and_vehicle_priority() {
    use earth_two_client::{character::Traversal, vehicle::Drivable};
    let mut a = app();
    let s = spot(&a, Kind::Sit);
    move_to(&mut a, s.approach());
    let p = a.world().resource::<Session>().player.unwrap();
    // Freeze fixed time so the input frame cannot land and undo the airborne setup.
    a.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    a.world_mut()
        .get_mut::<CharacterController>(p)
        .unwrap()
        .grounded = false;
    press(&mut a, KeyCode::KeyE);
    assert!(a.world().get::<FurnitureSeat>(p).is_none());
    a.world_mut()
        .get_mut::<CharacterController>(p)
        .unwrap()
        .grounded = true;
    a.world_mut().get_mut::<Traversal>(p).unwrap().mode = Anim::Vault;
    press(&mut a, KeyCode::KeyE);
    assert!(a.world().get::<FurnitureSeat>(p).is_none());
    *a.world_mut().get_mut::<Traversal>(p).unwrap() = default();
    *a.world_mut().get_mut::<Intent>(p).unwrap() = default();
    a.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1. / 60.,
    )));
    let car = a
        .world_mut()
        .query_filtered::<Entity, With<Drivable>>()
        .iter(a.world())
        .next()
        .unwrap();
    // A parked vehicle's source priority radius reserves E even outside its seat reach.
    let at = s.approach() + Vec3::X * 2.;
    a.world_mut()
        .entity_mut(car)
        .insert((Transform::from_translation(at), Position(at)));
    tick(&mut a, 2);
    press(&mut a, KeyCode::KeyE);
    assert!(a.world().get::<FurnitureSeat>(p).is_none());
    if a.world().get::<Seated>(p).is_some() {
        press(&mut a, KeyCode::KeyE);
    }
    assert!(
        a.world().get::<CharacterController>(p).is_some(),
        "vehicle priority may enter; exit before moving to contract"
    );
    let offer = a.world().resource::<Contracts>().offer;
    move_to(&mut a, offer);
    let overlapping = Spot {
        at: offer - Vec3::Z * 0.45,
        ..s
    };
    let mut uses = Uses::default();
    uses.cells.insert(
        ((offer.x / 8.).floor() as i32, (offer.z / 8.).floor() as i32),
        vec![overlapping],
    );
    a.insert_resource(uses);
    press(&mut a, KeyCode::KeyE);
    assert_eq!(screen(&a), Screen::ContractOffer);
    assert!(a.world().get::<FurnitureSeat>(p).is_none());
}
