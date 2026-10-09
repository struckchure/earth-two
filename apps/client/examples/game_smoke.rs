//! Opt-in graphical integration check, with synthetic game inputs. It never
//! sends OS input: ordinary play continues to use the platform input plugin.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    character::{CharacterController, Player},
    game::{GamePlugin, MenuAction, Screen, Session, StartAt},
    vehicle::Driving,
};

fn main() {
    std::fs::create_dir_all("build/migration-baseline/step-3-playable")
        .expect("create game smoke evidence directory");
    App::new()
        .add_plugins(
            DefaultPlugins.set(AssetPlugin {
                file_path: std::env::var("EARTH_TWO_ASSET_ROOT")
                    .unwrap_or_else(|_| earth_two_client::physics::source_assets()),
                ..default()
            }),
        )
        .insert_resource(StartAt::Buggy)
        .add_plugins(GamePlugin)
        .init_resource::<Review>()
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource, Default)]
struct Review {
    frame: usize,
    player: Option<Entity>,
    before: Vec3,
    parked: Vec3,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_scope(|world, mut review: Mut<Review>| {
        review.frame += 1;
        let frame = review.frame;
        let player = world.resource::<Session>().player.unwrap();
        for mut window in world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .iter_mut(world)
        {
            window.focused = true; // No focus dependency for this synthetic-input test.
        }
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        let key = match frame {
            120 | 540 => Some(KeyCode::KeyE),
            180..300 | 600..720 => Some(KeyCode::KeyW),
            300..540 => Some(KeyCode::Space),
            _ => None,
        };
        if let Some(key) = key {
            world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        }
        match frame {
            2 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Title);
                review.player = Some(player);
                world.write_message(MenuAction::Play);
            }
            180 => {
                let car = world.resource::<Driving>().vehicle.expect("entered buggy");
                assert!(world.get::<CharacterController>(player).is_none());
                review.before = world.get::<Transform>(car).unwrap().translation;
                info!("Game smoke: entered buggy");
            }
            300 => {
                let car = world.resource::<Driving>().vehicle.unwrap();
                assert!(
                    world
                        .get::<Transform>(car)
                        .unwrap()
                        .translation
                        .distance(review.before)
                        > 2.0
                );
                info!("Game smoke: drove buggy");
            }
            600 => {
                assert!(!world.resource::<Driving>().active(), "exit after braking");
                assert!(world.get::<CharacterController>(player).is_some());
                review.before = world.get::<Transform>(player).unwrap().translation;
            }
            720 => {
                assert!(
                    world
                        .get::<Transform>(player)
                        .unwrap()
                        .translation
                        .distance(review.before)
                        > 0.5
                );
                world.write_message(MenuAction::Pause);
                info!("Game smoke: exited and walked");
            }
            725 => review.parked = world.get::<Transform>(player).unwrap().translation,
            780 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Paused);
                assert_eq!(
                    world.get::<Transform>(player).unwrap().translation,
                    review.parked
                );
                world.write_message(MenuAction::MainMenu);
            }
            840 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Title);
                world.write_message(MenuAction::Play);
            }
            900 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Playing);
                assert_eq!(Some(player), review.player);
                assert_eq!(
                    world
                        .query_filtered::<Entity, With<Player>>()
                        .iter(world)
                        .count(),
                    1
                );
                info!("Game smoke passed: play, enter, drive, exit, walk, pause, title, resume");
                use bevy::render::view::window::screenshot::{
                    Screenshot, ScreenshotCaptured, save_to_disk,
                };
                world
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "build/migration-baseline/step-3-playable/native-play.png",
                    ))
                    .observe(
                        |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                            exit.write(AppExit::Success);
                        },
                    );
            }
            _ => {}
        }
    });
}
