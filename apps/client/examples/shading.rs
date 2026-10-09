//! The look, on its own: a crate and a man on the red ground under the low
//! sun, a lamp beside them and the Hull's light zone round the far end,
//! painted with the toon shader and the man inked.
//!
//! Run with `cargo run -p earth-two-client --example shading`; it captures
//! `build/rust/shading.png` once the models are in and quits, unless
//! `EARTH_TWO_STAY=1`.

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::WindowResolution,
    world_serialization::WorldInstanceReady,
};
use earth_two_client::{
    shading::{Outlined, ShadingPlugin, ShadowQuality, Smooth, Zone, sun_cascades},
    world::Lamp,
};

#[derive(Resource, Default)]
struct Loaded(u32);

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Earth Two — shading".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(ShadingPlugin::earth_two())
    .insert_resource(ShadowQuality::Full)
    .insert_resource(ClearColor(Color::srgb_u8(250, 196, 120)))
    .init_resource::<Loaded>()
    .add_observer(asset_ready)
    .add_systems(Startup, setup)
    .add_systems(Update, controls);
    if std::env::var_os("EARTH_TWO_STAY").is_none() {
        app.add_systems(Update, capture);
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Msaa::Sample4,
        Transform::from_xyz(5.0, 2.6, 7.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    // The low sun, from the south-west, as the game's daylight has it in
    // the afternoon.
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            shadow_depth_bias: 0.03,
            ..default()
        },
        sun_cascades(),
        Transform::from_xyz(-6.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // The red ground, lit smoothly.
    commands.spawn((
        Name::new("ground"),
        Smooth,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(176, 92, 64),
            ..default()
        })),
    ));
    commands.spawn((
        Name::new("crate"),
        Transform::from_xyz(-1.4, 0.0, 0.0),
        Visibility::default(),
        children![WorldAssetRoot(assets.load("world/crate.glb#Scene0"))],
    ));
    commands.spawn((
        Name::new("man"),
        Outlined,
        Transform::from_xyz(1.2, 0.0, 0.6).looking_to(Vec3::new(0.6, 0.0, 1.0), Vec3::Y),
        Visibility::default(),
        children![WorldAssetRoot(assets.load("characters/man.glb#Scene0"))],
    ));
    // A work lamp, as the layouts place them.
    commands.spawn((
        Name::new("lamp"),
        Lamp {
            color: [1.0, 0.76, 0.5],
            intensity: 2.0,
            range: 9.0,
        },
        Transform::from_xyz(0.0, 2.6, -1.2),
        Visibility::default(),
    ));
    // The Hull's light zone, over the far side, to see its fill take over.
    commands.insert_resource(zoned_look());
}

/// The game's look with one of its zones moved here: the Hull's warm fill
/// from x = 2 on.
fn zoned_look() -> earth_two_client::shading::Look {
    let mut look = earth_two_client::shading::Look::earth_two();
    let hull = look.zones[1];
    look.zones = vec![Zone {
        min: Vec3::new(2.0, -5.0, -6.0),
        max: Vec3::new(20.0, 11.0, 6.0),
        ..hull
    }];
    look
}

fn asset_ready(
    event: On<WorldInstanceReady>,
    roots: Query<&WorldAssetRoot>,
    mut loaded: ResMut<Loaded>,
) {
    if roots.get(event.entity).is_ok() {
        loaded.0 += 1;
    }
}

/// Captures the frame to build/rust/shading.png once both models are in
/// and the shadow map and pipelines have settled, then quits.
fn capture(
    mut commands: Commands,
    loaded: Res<Loaded>,
    time: Res<Time<Real>>,
    mut requested: Local<bool>,
) {
    if *requested || loaded.0 < 2 || time.elapsed_secs() < 4.0 {
        return;
    }
    *requested = true;
    let path = std::path::absolute("build/rust/shading.png").expect("a working directory");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("build/rust must be writable");
    }
    info!("Capturing to {}", path.display());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

/// 1, 2, 3: full, low and no shadows.
fn controls(keys: Res<ButtonInput<KeyCode>>, mut quality: ResMut<ShadowQuality>) {
    for (key, q) in [
        (KeyCode::Digit1, ShadowQuality::Full),
        (KeyCode::Digit2, ShadowQuality::Low),
        (KeyCode::Digit3, ShadowQuality::Off),
    ] {
        if keys.just_pressed(key) {
            *quality = q;
        }
    }
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
