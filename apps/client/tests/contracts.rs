//! Go contracts_test.go scenarios exercised through the live Landfall schedule.
#![cfg(not(feature = "viewer"))]
use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{Anim, CharacterController, Controls, Intent, Traversal},
    game::{
        GamePlugin, MenuAction, Screen, Session, StartAt,
        camera::GameCamera,
        contracts::{self, ContractState, Contracts, first_work_order},
        menu::Menu,
        navigation::MapPointer,
    },
    landfall::{ground::Rectangle, maps::WorldMap},
    physics::source_assets,
    presentation::{MotionSamples, Roster, content, roster::Skin},
    vehicle::Prompt,
};
use earth_two_world::kit::{Kit, Layout};
use std::time::Duration;
fn jobs() -> Contracts {
    let root = source_assets();
    Contracts::new(
        &Kit::load(&root, "world/world.json").unwrap(),
        &Layout::load(&root, "world/landfall.json").unwrap().pieces,
    )
    .unwrap()
}
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
#[test]
fn first_journey_reviews_declines_accepts_delivers_and_preserves_session() {
    let mut a = app();
    let offer = a.world().resource::<Contracts>().offer;
    let delivery = a.world().resource::<Contracts>().delivery;
    move_to(&mut a, delivery);
    press(&mut a, KeyCode::KeyE);
    assert_eq!(
        screen(&a),
        Screen::Playing,
        "delivery before acceptance does nothing"
    );
    move_to(&mut a, offer);
    assert_eq!(a.world().resource::<Prompt>().text, "Review Ada's contract");
    press(&mut a, KeyCode::KeyE);
    assert_eq!(screen(&a), Screen::ContractOffer);
    assert!(!a.world().resource::<Controls>().enabled);
    assert!(a.world().resource::<Time<Physics>>().is_paused());
    let p = a.world().resource::<Session>().player.unwrap();
    assert_eq!(a.world().get::<Intent>(p).unwrap().act, Anim::Idle);
    let at = a.world().get::<Transform>(p).unwrap().translation;
    tick(&mut a, 20);
    assert_eq!(a.world().get::<Transform>(p).unwrap().translation, at);
    press(&mut a, KeyCode::ArrowDown);
    press(&mut a, KeyCode::Enter);
    assert_eq!(screen(&a), Screen::Playing);
    assert!(a.world().resource::<Contracts>().available());
    press(&mut a, KeyCode::KeyJ);
    press(&mut a, KeyCode::Enter);
    action(&mut a, MenuAction::AcceptContract); // remote acceptance is not allowed
    assert!(a.world().resource::<Contracts>().available());
    press(&mut a, KeyCode::KeyE);
    press(&mut a, KeyCode::Escape);
    assert!(a.world().resource::<Contracts>().available());
    press(&mut a, KeyCode::KeyE);
    press(&mut a, KeyCode::Enter);
    assert_eq!(screen(&a), Screen::Playing);
    assert_eq!(
        a.world().resource::<Contracts>().state,
        ContractState::Accepted
    );
    assert_eq!(a.world().resource::<Contracts>().debt, 2000);
    press(&mut a, KeyCode::KeyJ);
    press(&mut a, KeyCode::KeyJ);
    assert_eq!(screen(&a), Screen::Playing);
    move_to(&mut a, delivery);
    assert_eq!(
        a.world().resource::<Contracts>().state,
        ContractState::Accepted,
        "arrival never auto-delivers"
    );
    assert_eq!(
        a.world().resource::<Prompt>().text,
        "Hand over the sealed filing"
    );
    press(&mut a, KeyCode::KeyE);
    assert_eq!(screen(&a), Screen::ContractJournal);
    let settled = a.world().resource::<Contracts>().clone();
    assert_eq!(
        (settled.balance, settled.debt, settled.completed.len()),
        (0, 1850, 1)
    );
    assert!(settled.ongoing.is_none());
    assert_eq!(settled.completed[0].debt_credit, 150);
    press(&mut a, KeyCode::KeyJ);
    press(&mut a, KeyCode::KeyE);
    action(&mut a, MenuAction::Pause);
    action(&mut a, MenuAction::MainMenu);
    action(&mut a, MenuAction::Play);
    assert_eq!(
        *a.world().resource::<Contracts>(),
        settled,
        "title/resume preserves the record"
    );
}
#[test]
fn receipt_preserves_terms_caps_credit_and_never_pays_cash_or_twice() {
    let mut c = jobs();
    c.balance = 73;
    c.debt = 80;
    assert!(c.accept());
    let terms = c.ongoing.clone().unwrap();
    assert!(!c.deliver(c.delivery + Vec3::Y * 1.41));
    assert!(c.deliver(c.delivery));
    let r = &c.completed[0];
    assert_eq!(
        (&r.id, &r.title, &r.poster, &r.objective, r.pay),
        (
            &terms.id,
            &terms.title,
            &terms.poster,
            &terms.objective,
            terms.pay
        )
    );
    assert_eq!((r.debt_credit, c.debt, c.balance), (80, 0, 73));
    assert!(!c.deliver(c.delivery));
    assert!(!c.accept());
    assert_eq!(c.completed.len(), 1);
    let mut occupied = jobs();
    occupied.ongoing = Some(terms.clone());
    assert!(!occupied.accept());
    assert_eq!(occupied.ongoing, Some(terms));
}
#[test]
fn contract_guidance_preserves_manual_markers_and_does_not_arrive_automatically() {
    let mut c = jobs();
    let mut m = WorldMap::empty(Rectangle::new(-16000., -16000., 32000., 32000.));
    assert_eq!(
        contracts::route(&m, Some(&c)),
        Some((c.offer.xz(), "Arrivals terminal"))
    );
    assert!(!m.marked);
    assert!(c.accept());
    assert_eq!(
        contracts::route(&m, Some(&c)),
        Some((c.delivery.xz(), "Exchange delivery"))
    );
    earth_two_client::landfall::maps::arrive(&mut m, c.delivery.xz());
    assert!(contracts::route(&m, Some(&c)).is_some());
    m.marked = true;
    m.dest = Vec2::new(20., 30.);
    assert_eq!(
        contracts::route(&m, Some(&c)),
        Some((m.dest, "Destination"))
    );
    assert!(c.deliver(c.delivery));
    assert!(contracts::route(&m, Some(&c)).is_some());
    m.marked = false;
    assert!(contracts::route(&m, Some(&c)).is_none());
}
#[test]
fn click_card_and_badge_only_open_journal_and_history_scroll_is_bounded() {
    let mut a = app();
    for badge in [false, true] {
        let size = Vec2::new(1280., 720.);
        let (card, b) = contracts::account_rects(size);
        {
            let mut p = a.world_mut().resource_mut::<MapPointer>();
            p.size = size;
            p.at = Some(if badge {
                b.min + Vec2::splat(2.)
            } else {
                card.center()
            });
            p.pressed = true;
        }
        tick(&mut a, 1);
        a.world_mut().resource_mut::<MapPointer>().pressed = false;
        tick(&mut a, 1);
        assert_eq!(screen(&a), Screen::ContractJournal);
        assert!(a.world().resource::<Contracts>().available());
        press(&mut a, KeyCode::Enter);
        assert_eq!(screen(&a), Screen::Playing);
    }
    {
        let mut c = a.world_mut().resource_mut::<Contracts>();
        for i in 0..5 {
            let mut r = first_work_order();
            r.id = format!("R{i}");
            c.completed.push(r);
        }
        c.balance = 73;
        c.debt = 1600;
    }
    let before = a.world().resource::<Contracts>().clone();
    press(&mut a, KeyCode::KeyJ);
    let size = Vec2::new(1280., 720.);
    let (sc, _, history) = contracts::journal_layout(size);
    {
        let mut p = a.world_mut().resource_mut::<MapPointer>();
        p.at = Some(history.center() * sc);
        p.wheel = -1.;
    }
    tick(&mut a, 8);
    assert_eq!(
        a.world().resource::<Menu>().stack.last().unwrap().receipt,
        1
    );
    a.world_mut().resource_mut::<MapPointer>().wheel = 1.;
    tick(&mut a, 8);
    assert_eq!(
        a.world().resource::<Menu>().stack.last().unwrap().receipt,
        0
    );
    assert_eq!(*a.world().resource::<Contracts>(), before);
}
#[test]
fn contract_needs_free_grounded_player_and_preserves_vehicle_priority() {
    let mut a = app();
    let offer = a.world().resource::<Contracts>().offer;
    for condition in [
        "far",
        "upstairs",
        "airborne",
        "traversal",
        "paused",
        "vehicle",
    ] {
        move_to(&mut a, offer);
        let p = a.world().resource::<Session>().player.unwrap();
        let mut car = None;
        match condition {
            "far" => a.world_mut().get_mut::<Transform>(p).unwrap().translation.x += 3.,
            "upstairs" => a.world_mut().get_mut::<Transform>(p).unwrap().translation.y += 3.6,
            "airborne" => {
                a.world_mut()
                    .get_mut::<CharacterController>(p)
                    .unwrap()
                    .grounded = false
            }
            "traversal" => a.world_mut().get_mut::<Traversal>(p).unwrap().mode = Anim::Vault,
            "paused" => action(&mut a, MenuAction::Pause),
            "vehicle" => {
                // Move a real parked car into the broad Go vehicle-priority radius.
                let (e,tr)=a.world_mut().query_filtered::<(Entity,&Transform),With<earth_two_client::vehicle::Drivable>>().iter(a.world()).next().map(|(e,t)|(e,*t)).unwrap();
                car = Some((e, tr));
                a.world_mut().get_mut::<Transform>(e).unwrap().translation = offer + Vec3::X * 3.;
            }
            _ => unreachable!(),
        }
        // Zero elapsed time preserves the deliberately injected airborne/traversal boundary.
        a.world_mut()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        press(&mut a, KeyCode::KeyE);
        assert_ne!(screen(&a), Screen::ContractOffer, "{condition}");
        assert!(a.world().resource::<Contracts>().available(), "{condition}");
        if let Some((e, tr)) = car {
            a.world_mut().entity_mut(e).insert(tr);
        }
        *a.world_mut().get_mut::<Traversal>(p).unwrap() = default();
        if screen(&a) == Screen::Paused {
            action(&mut a, MenuAction::Resume);
        }
        a.world_mut()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1. / 60.,
            )));
    }
}
#[test]
fn missing_contract_landmarks_fail_instead_of_silently_omitting_gameplay() {
    assert!(
        Contracts::new(&Kit::default(), &[])
            .unwrap_err()
            .contains("terminal_kiosk")
    );
    let root = source_assets();
    let k = Kit::load(&root, "world/world.json").unwrap();
    let mut l = Layout::load(&root, "world/landfall.json").unwrap();
    l.pieces.retain(|p| p.piece != "registrar_counter");
    assert!(
        Contracts::new(&k, &l.pieces)
            .unwrap_err()
            .contains("registrar_counter")
    );
}
