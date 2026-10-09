//! Rendered resident lifecycle check; injects game input without OS events.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    character::Body,
    game::{
        GamePlugin, MenuAction, Screen, Session, StartAt,
        crowd::TestCrowd,
        residents::{ResidentOf, Residents},
    },
    presentation::viewer::MeshEntities,
};
fn main() {
    std::fs::create_dir_all("build/migration-baseline/step-4-residents").unwrap();
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(StartAt::Crowd)
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .init_resource::<Review>()
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource, Default)]
struct Review {
    stage: u8,
    since: f32,
    frames: u32,
    frozen: Vec<(Entity, Vec3)>,
    owned: Vec<Entity>,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut windows {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}
fn capture(world: &mut World, name: &str, finish: bool) {
    use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let mut shot = world.spawn(Screenshot::primary_window());
    shot.observe(save_to_disk(format!(
        "build/migration-baseline/step-4-residents/native-{name}.png"
    )));
    if finish {
        shot.observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
    }
}
fn key(world: &mut World, key: KeyCode) {
    world.resource_mut::<ButtonInput<KeyCode>>().press(key);
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready {
        return;
    }
    world.resource_scope(|world, mut review: Mut<Review>| {
        let now = world.resource::<Time<Real>>().elapsed_secs();
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        for mut w in world.query_filtered::<&mut Window, With<PrimaryWindow>>().iter_mut(world) { w.focused = true; }
        review.frames += 1;
        assert!(review.frames < 10000, "crowd smoke timed out in stage {}", review.stage);
        let count = world.query::<&ResidentOf>().iter(world).count();
        match review.stage {
            0 if *world.resource::<State<Screen>>().get() == Screen::Title => {
                world.resource_mut::<earth_two_client::game::camera::Orbit>().yaw = std::f32::consts::PI;
                world.write_message(MenuAction::Play); review.stage = 1; review.since = now;
            }
            1 if count == 25 && now - review.since > 8. && world.query_filtered::<&MeshEntities, With<Body>>().iter(world).count() == 26 => {
                assert_eq!(world.resource::<Residents>().near, 25);
                let far = world.query::<&earth_two_client::game::people::Detail>().iter(world).filter(|d| d.0.distant).count();
                assert!(far > 0, "live distance budgets were not applied");
                capture(world, "crowd", false);
                review.stage = 2; review.since = now;
            }
            2 if now - review.since > 0.5 => { world.write_message(MenuAction::Pause); review.stage = 3; }
            3 if *world.resource::<State<Screen>>().get() == Screen::Paused => {
                review.frozen = world.query_filtered::<(Entity, &Transform), With<ResidentOf>>().iter(world).map(|(e,t)| (e,t.translation)).collect();
                review.since = now; review.stage = 4;
            }
            4 if now - review.since > 0.5 => {
                for (e, at) in &review.frozen { assert_eq!(world.get::<Transform>(*e).unwrap().translation, *at); }
                world.write_message(MenuAction::Resume); review.stage = 5;
            }
            5 if *world.resource::<State<Screen>>().get() == Screen::Playing => { key(world, KeyCode::F9); review.stage = 6; }
            6 => { assert!(world.resource::<TestCrowd>().editing); key(world, KeyCode::Digit3); review.stage = 7; }
            7 => { key(world, KeyCode::Enter); review.stage = 8; }
            8 if count == 3 => {
                assert_eq!(world.resource::<TestCrowd>().population, 3);
                review.owned = world.query_filtered::<Entity, With<ResidentOf>>().iter(world).collect();
                let mut i = 0;
                while i < review.owned.len() { if let Some(children) = world.get::<Children>(review.owned[i]) { review.owned.extend(children.iter()); } i += 1; }
                key(world, KeyCode::F8); review.stage = 9;
            }
            9 if count == 0 => {
                assert_eq!(world.query_filtered::<Entity, With<Body>>().iter(world).count(), 1);
                for e in &review.owned { assert!(world.get_entity(*e).is_err(), "orphaned resident descendant {e:?}"); }
                key(world, KeyCode::F8); review.stage = 10; review.since = now;
            }
            10 if count == 3 && now - review.since > 3. && world.query_filtered::<&MeshEntities, With<Body>>().iter(world).count() == 4 => {
                capture(world, "recreated", true);
                info!("Crowd smoke passed: 25 loaded actors, detail budgets, pause, F9 shrink, F8 off/on and recursive cleanup"); review.stage = 11;
            }
            _ => {}
        }
    });
}
