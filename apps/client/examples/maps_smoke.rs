//! Finite graphical map/teleport review using the application's input and render schedules.
use bevy::{
    input::{
        InputSystems,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{
        GamePlugin, GameSet, Screen, Session, StartAt, camera::GameCamera, navigation::full_frame,
    },
    landfall::maps::WorldMap,
    vehicle::Driving,
};
const EVIDENCE: &str = "build/migration-baseline/step-8-maps";
fn main() {
    std::fs::create_dir_all(EVIDENCE).unwrap();
    unsafe {
        std::env::set_var(
            "EARTH_TWO_IDENTITY_PATH",
            format!("{EVIDENCE}/native-profile/pk"),
        );
    }
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(StartAt::Hauler)
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .init_resource::<Review>()
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release.after(GameSet::Menu))
        .run();
}
#[derive(Resource, Default)]
struct Review {
    frame: usize,
    at: Vec2,
    marked: Vec2,
    car: Option<Entity>,
}
fn release(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut windows {
        c.grab_mode = CursorGrabMode::None;
    }
}
fn capture(world: &mut World, name: &str) {
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{EVIDENCE}/native-{name}.png")));
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    world.resource_scope(|world, mut r: Mut<Review>| {
        r.frame += 1;
        let size = {
            let mut w = world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(world)
                .unwrap();
            w.focused = true;
            let size = Vec2::new(w.width(), w.height());
            w.set_cursor_position(Some(size / 2. + Vec2::new(100., 60.)));
            size
        };
        if (780..900).contains(&r.frame) {
            let target =
                full_frame(world.resource::<WorldMap>(), size).to_screen(Vec2::new(9600., -1700.));
            world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(world)
                .unwrap()
                .set_cursor_position(Some(target));
        }
        let key = match r.frame {
            30 => Some(KeyCode::Enter),
            150 | 400 | 750 => Some(KeyCode::KeyM),
            500 => Some(KeyCode::KeyP),
            620 => Some(KeyCode::KeyE),
            850 => Some(KeyCode::KeyP),
            _ => None,
        };
        if let Some(k) = key {
            world.resource_mut::<ButtonInput<KeyCode>>().press(k);
        }
        match r.frame {
            140 => capture(world, "minimap"),
            200 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Mapping);
                assert!(
                    world
                        .query::<(&Text, &ComputedNode)>()
                        .iter(world)
                        .any(|(t, n)| t.0 == "The Fringe" && n.size().x > 0.)
                );
                assert!(
                    !world
                        .query::<(&Text, &ComputedNode)>()
                        .iter(world)
                        .any(|(t, n)| t.0 == "N" && n.size().x > 0.),
                    "compass labels hidden on full map"
                );
                capture(world, "full-map");
            }
            220 => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
            }
            221 => {
                let mut b = world.resource_mut::<ButtonInput<MouseButton>>();
                b.press(MouseButton::Left);
                b.clear();
                b.release(MouseButton::Left);
            }
            240 => {
                assert!(world.resource::<WorldMap>().marked);
                r.marked = world.resource::<WorldMap>().dest;
                capture(world, "destination");
            }
            260 => {
                world.write_message(MouseWheel {
                    unit: MouseScrollUnit::Line,
                    x: 0.,
                    y: 30.,
                    window: Entity::PLACEHOLDER,
                    phase: bevy::input::touch::TouchPhase::Moved,
                });
            }
            300 => capture(world, "zoomed"),
            320..340 => {
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Right);
                let pointer =
                    size / 2. + Vec2::new(100., 60.) + Vec2::new(4., -2.) * (r.frame - 319) as f32;
                world
                    .query_filtered::<&mut Window, With<PrimaryWindow>>()
                    .single_mut(world)
                    .unwrap()
                    .set_cursor_position(Some(pointer));
            }
            360 => {
                assert_eq!(world.resource::<WorldMap>().dest, r.marked);
                capture(world, "panned");
            }
            430 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Playing);
                capture(world, "navigation");
            }
            510 => {
                let e = world.resource::<Session>().player.unwrap();
                assert!(
                    world
                        .get::<Transform>(e)
                        .unwrap()
                        .translation
                        .xz()
                        .distance(r.marked)
                        < 1.
                );
            }
            570 => {
                assert!(!world.resource::<WorldMap>().marked);
                capture(world, "teleported");
                // Put the player beside the original hauler for the second teleport.
                let e = world.resource::<Session>().player.unwrap();
                let (car, at) = world
                    .query::<(Entity, &earth_two_client::vehicle::Drivable, &Transform)>()
                    .iter(world)
                    .filter(|(_, d, _)| d.name == "hauler")
                    .map(|(e, d, t)| {
                        let (p, _) = earth_two_client::vehicle::spec::seat_pose(
                            t.translation,
                            t.rotation,
                            &d.spec.seats[0],
                        );
                        (e, Vec3::new(p.x + 1.6, t.translation.y + 0.95, p.z))
                    })
                    .min_by(|(_, a), (_, b)| {
                        a.distance_squared(earth_two_world::terrain::ARRIVAL)
                            .total_cmp(&b.distance_squared(earth_two_world::terrain::ARRIVAL))
                    })
                    .unwrap();
                world.get_mut::<Transform>(e).unwrap().translation = at;
                world.get_mut::<avian3d::prelude::Position>(e).unwrap().0 = at;
                r.car = Some(car);
            }
            680 => {
                assert_eq!(world.resource::<Driving>().vehicle, r.car);
                capture(world, "hauler-navigation");
            }
            800 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Mapping);
                r.at = Vec2::new(9600., -1700.);
                capture(world, "hauler-map");
            }
            852 => {
                let at = world
                    .get::<Transform>(r.car.unwrap())
                    .unwrap()
                    .translation
                    .xz();
                assert!(
                    at.distance(r.at) < 0.5,
                    "teleported immediately: {at:?} vs {:?}",
                    r.at
                );
            }
            930 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Playing);
                assert_eq!(world.resource::<Driving>().vehicle, r.car);
                let car = world.get::<Transform>(r.car.unwrap()).unwrap();
                assert!(
                    car.translation.xz().distance(r.at) < 2.,
                    "settled: {:?} vs {:?}",
                    car.translation,
                    r.at
                );
                let camera = world
                    .query_filtered::<&Transform, With<GameCamera>>()
                    .single(world)
                    .unwrap();
                assert!(camera.translation.xz().distance(r.at) < 40.);
                capture(world, "hauler-teleported");
            }
            980 => {
                info!(
                    "Map review passed: minimap, compass, mark, zoom, pan, foot and hauler teleport"
                );
                world.write_message(AppExit::Success);
            }
            _ => {}
        }
    });
}
