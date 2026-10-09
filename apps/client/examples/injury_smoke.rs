//! Rendered injury/recovery regression using the opt-in Go-compatible review
//! knockdown. Synthetic game input only; no OS keyboard or mouse injection.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    character::{CharacterController, Downed, Health, LifeState, Player, Ragdoll},
    game::{GamePlugin, MenuAction, Screen, Session, StartAt},
    presentation::pose::PoseFit,
};

fn main() {
    std::fs::create_dir_all("build/migration-baseline/step-4-injuries").unwrap();
    let fatal = std::env::args().any(|a| a == "--fatal");
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(if fatal {
            StartAt::Fatal
        } else {
            StartAt::Injury
        })
        .add_plugins(GamePlugin)
        .insert_resource(Review { fatal, ..default() })
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource, Default)]
struct Review {
    stage: u8,
    since: f32,
    frozen: Vec3,
    fatal: bool,
    frames: u32,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
fn capture(world: &mut World, name: &str, fatal: bool, finish: bool) {
    use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let path = format!(
        "build/migration-baseline/step-4-injuries/native-{}-{name}.png",
        if fatal { "fatal" } else { "critical" }
    );
    let mut screenshot = world.spawn(Screenshot::primary_window());
    screenshot.observe(save_to_disk(path));
    if finish {
        screenshot.observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
    }
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready {
        return;
    }
    world.resource_scope(|world, mut review: Mut<Review>| {
        let player = world.resource::<Session>().player.unwrap();
        let now = world.resource::<Time<Real>>().elapsed_secs();
        for mut window in world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .iter_mut(world)
        {
            window.focused = true;
        }
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        review.frames += 1;
        assert!(
            review.frames < 10000,
            "injury smoke timed out in stage {}",
            review.stage
        );
        match review.stage {
            0 if *world.resource::<State<Screen>>().get() == Screen::Title => {
                world.write_message(MenuAction::Play);
                review.stage = 1;
                review.since = now;
            }
            1 if world.get::<Downed>(player).is_some() => {
                assert_eq!(
                    world.get::<Health>(player).unwrap().state,
                    if review.fatal {
                        LifeState::Dead
                    } else {
                        LifeState::Critical
                    }
                );
                review.stage = 2;
                review.since = now;
                info!(
                    "Injury smoke: entered {:?}",
                    world.get::<Health>(player).unwrap().state
                );
            }
            2 if now - review.since > 6.5
                && world
                    .query::<&PoseFit>()
                    .iter(world)
                    .any(|p| p.downed && p.frames > 30) =>
            {
                let ragdoll = world.get::<Ragdoll>(player).unwrap();
                assert!(
                    ragdoll.rig.as_ref().is_some_and(|r| r.point_count() >= 14),
                    "no articulated live skeleton"
                );
                assert!(
                    world
                        .query::<&PoseFit>()
                        .iter(world)
                        .any(|p| p.downed && p.frames > 30)
                );
                capture(world, "fallen", review.fatal, false);
                review.stage = 8;
                review.since = now;
            }
            8 if now - review.since > 0.3 => {
                world.write_message(MenuAction::Pause);
                review.stage = 3;
                review.since = now;
            }
            3 if *world.resource::<State<Screen>>().get() == Screen::Paused => {
                review.frozen = world.get::<Transform>(player).unwrap().translation;
                review.stage = 4;
                review.since = now;
            }
            4 if now - review.since > 0.5 => {
                assert_eq!(
                    review.frozen,
                    world.get::<Transform>(player).unwrap().translation
                );
                world.write_message(MenuAction::Resume);
                review.stage = 5;
                review.since = now;
            }
            5 if *world.resource::<State<Screen>>().get() == Screen::Playing => {
                world
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyR);
                review.stage = 6;
                review.since = now;
            }
            6 if now - review.since > 0.5 => {
                assert!(
                    world.get::<Downed>(player).is_none() && world.get::<Ragdoll>(player).is_none()
                );
                assert!(world.get::<CharacterController>(player).is_some());
                assert!(world.query::<&PoseFit>().iter(world).all(|p| !p.downed));
                assert_eq!(
                    world.get::<Health>(player).unwrap().state,
                    LifeState::Healthy
                );
                assert_eq!(
                    world
                        .query_filtered::<Entity, With<Player>>()
                        .iter(world)
                        .count(),
                    1
                );
                capture(world, "recovered", review.fatal, true);
                info!("Injury smoke passed: fall, articulated pose, pause, recovery and cleanup");
                review.stage = 7;
            }
            _ => {}
        }
    });
}
