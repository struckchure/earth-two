//! A dressed, animated character: the man in the starting outfit, walking
//! towards the camera on the red ground, inked and painted by the shading
//! plugin, his clothes on the desktop cloth solver.
//!
//! Run with `cargo run -p earth-two-client --example presentation`; it
//! captures `build/rust/presentation.png` once the models are in and
//! quits, unless `EARTH_TWO_STAY=1`.

use avian3d::prelude::*;
use bevy::{
    asset::AssetPlugin,
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::WindowResolution,
};
use earth_two_client::{
    character::{CharacterPlugin, Intent},
    physics::EarthPhysicsPlugin,
    presentation::{
        PresentationPlugin, Roster, Wardrobe, content::starting_outfit, viewer::Content,
    },
    shading::{ShadingPlugin, ShadowQuality, Smooth},
};

fn main() {
    let root = asset_root();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: root.clone(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Earth Two — presentation".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((
        EarthPhysicsPlugin,
        CharacterPlugin,
        ShadingPlugin::earth_two(),
        PresentationPlugin::platform(),
    ))
    .insert_resource(Content::earth_two(&root))
    .insert_resource(ShadowQuality::Full)
    .insert_resource(ClearColor(Color::srgb_u8(250, 196, 120)))
    .add_systems(Startup, setup)
    .add_systems(Update, spawn_person);
    if std::env::var_os("EARTH_TWO_STAY").is_none() {
        app.add_systems(Update, capture);
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Msaa::Sample4,
        Transform::from_xyz(1.6, 1.7, 3.6).looking_at(Vec3::new(0.0, 1.0, -0.5), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            shadow_depth_bias: 0.03,
            ..default()
        },
        Transform::from_xyz(-6.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Name::new("ground"),
        Smooth,
        RigidBody::Static,
        Collider::cuboid(80.0, 0.2, 80.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
        Mesh3d(meshes.add(Cuboid::new(80.0, 0.2, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(176, 92, 64),
            ..default()
        })),
    ));
}

#[derive(Component)]
struct Person;

/// spawn_person spawns the man once the roster and wardrobe are in, in the
/// starting outfit, walking towards the camera.
fn spawn_person(
    mut commands: Commands,
    roster: Res<Roster>,
    wardrobe: Res<Wardrobe>,
    people: Query<(), With<Person>>,
) {
    if roster.skins.is_empty() || wardrobe.bodies.is_empty() || !people.is_empty() {
        return;
    }
    let root = roster.spawn(&mut commands, 0, Vec3::new(-0.6, 0.0, -5.5), 0.0);
    commands.entity(root).insert((
        Person,
        starting_outfit(&wardrobe),
        Intent {
            move_dir: Vec3::new(0.1, 0.0, 1.0).normalize(),
            ..default()
        },
    ));
}

/// capture takes the screenshot once the models have had time to load and
/// the man has walked into view.
fn capture(
    mut commands: Commands,
    time: Res<Time<Real>>,
    people: Query<(), With<Person>>,
    mut requested: Local<bool>,
) {
    if *requested || people.is_empty() || time.elapsed_secs() < 5.0 {
        return;
    }
    *requested = true;
    let path = "build/rust/presentation.png";
    info!("Capturing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

fn asset_root() -> String {
    let path = std::env::var_os("EARTH_TWO_ASSET_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("assets"));
    std::path::absolute(path)
        .expect("asset directory must resolve")
        .to_string_lossy()
        .into_owned()
}
