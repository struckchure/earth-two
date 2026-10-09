//! A development scene for checking engine integration and existing assets.
//! These default materials are temporary; the game's shaders are a later port.

use avian3d::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy::{
    asset::AssetPlugin,
    light::DirectionalLightShadowMap,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

use crate::{
    physics::EarthPhysicsPlugin,
    scenes::{Ground, Probe, foundation_scene},
    world::{Lamp, LayoutAsset, LayoutRoot, PieceModel, Placed, SpawnLayout, Wheel, WorldPlugin},
};

#[derive(Resource, Default)]
struct LoadedScenes(u32);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct CapturePath(String);

/// The manifest's light intensity is the game shader's own scale, not
/// lumens. This stand-in keeps lamps visible until the shading port maps
/// them properly; it claims no parity.
const LAMP_LUMENS_PER_UNIT: f32 = 40_000.0;

pub fn run() {
    let mut app = App::new();
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(path) = std::env::var("EARTH_TWO_CAPTURE") {
        app.insert_resource(CapturePath(path))
            .add_systems(Update, capture);
    }
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Earth Two — Rust foundation".into(),
                    resolution: resolution(),
                    present_mode: PresentMode::AutoVsync,
                    canvas: Some("#earth-two".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((EarthPhysicsPlugin, WorldPlugin))
    .init_resource::<LoadedScenes>()
    .add_observer(asset_ready)
    .insert_resource(DirectionalLightShadowMap {
        size: if cfg!(target_arch = "wasm32") {
            1024
        } else {
            4096
        },
    })
    .insert_resource(ClearColor(Color::srgb(0.22, 0.16, 0.20)))
    .add_systems(
        Startup,
        (foundation_scene.spawn(), showcase.spawn(), request_block),
    )
    .add_systems(
        Update,
        (
            attach_probe_meshes,
            attach_pieces,
            attach_lamps,
            controls,
            report_ready,
        ),
    )
    .run();
}

/// The Hull test block, placed from the same manifests the Go client uses.
fn request_block(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<LayoutAsset>>,
) {
    // This development layout is intentionally absent from Go's release pack.
    // Embed only its placement manifest; models/textures still load from the
    // selected asset root, including the unmodified packed release assets.
    let layout = earth_two_world::kit::Layout::parse(include_bytes!(
        "../../../assets/world/hull_block.json"
    ))
    .expect("valid foundation layout");
    commands.spawn(SpawnLayout {
        kit: assets.load("world/world.json"),
        layout: layouts.add(LayoutAsset(layout)),
    });
}

fn asset_ready(
    event: On<WorldInstanceReady>,
    roots: Query<&WorldAssetRoot>,
    mut loaded: ResMut<LoadedScenes>,
) {
    if let Ok(root) = roots.get(event.entity) {
        loaded.0 += 1;
        debug!("Scene asset ready: {:?}", root.0.path());
    }
}

/// A runtime check shared by native and browser captures. Compilation alone
/// does not verify asynchronous assets or an actual browser physics step.
fn report_ready(
    loaded: Res<LoadedScenes>,
    roots: Query<&WorldAssetRoot>,
    requests: Query<(), With<SpawnLayout>>,
    probes: Query<(&Position, &LinearVelocity), With<Probe>>,
    mut reported: Local<bool>,
) {
    if *reported || !requests.is_empty() || roots.iter().count() < 3 {
        return;
    }
    let Ok((probe, velocity)) = probes.single() else {
        return;
    };
    // The asynchronously placed Hull can catch the probe above the plain
    // floor. Require it to have fallen from y=3 and settled, not a fixed height.
    if loaded.0 as usize >= roots.iter().count()
        && (0.3..2.9).contains(&probe.y)
        && velocity.length() < 0.05
    {
        info!(
            "Foundation ready: {} loaded scenes; physics probe y={:.4}",
            loaded.0, probe.y
        );
        *reported = true;
    }
}

/// Native capture hook for repeatable migration evidence, after every
/// requested model has loaded.
#[cfg(not(target_arch = "wasm32"))]
fn capture(
    mut commands: Commands,
    path: Res<CapturePath>,
    loaded: Res<LoadedScenes>,
    roots: Query<&WorldAssetRoot>,
    time: Res<Time<Real>>,
    mut requested: Local<bool>,
) {
    use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let requested_models = roots.iter().count() as u32;
    if *requested
        || requested_models < 3
        || loaded.0 < requested_models
        || time.elapsed_secs() < 5.0
    {
        return;
    }
    *requested = true;
    info!("Captured {} loaded scenes to {}", loaded.0, path.0);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.0.clone()))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

fn resolution() -> WindowResolution {
    let resolution = WindowResolution::new(1280, 720);
    if cfg!(target_arch = "wasm32") {
        resolution.with_scale_factor_override(1.0)
    } else {
        resolution
    }
}

fn msaa() -> Msaa {
    if cfg!(target_arch = "wasm32") {
        Msaa::Off
    } else {
        Msaa::Sample4
    }
}

fn asset_root() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "assets".into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::env::var_os("EARTH_TWO_ASSET_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("assets"));
        std::path::absolute(path)
            .expect("asset directory must resolve")
            .to_string_lossy()
            .into_owned()
    }
}

fn showcase() -> impl SceneList {
    bsn_list! {
        Camera3d::default()
        msaa()
        Transform::from_xyz(16.0, 11.0, 20.0).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y)
        --
        DirectionalLight { illuminance: 10000.0, shadow_maps_enabled: true }
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)
        --
        @model("characters/man.glb#Scene0", Vec3::new(10.0, 0.0, 2.0))
        --
        @model("world/buggy.glb#Scene0", Vec3::new(12.0, 0.0, -4.0))
        --
        @panel()
    }
}

fn model(path: &'static str, position: Vec3) -> impl Scene {
    bsn! {
        Name::new(path)
        Transform::from_translation(position)
        Visibility::default()
        Children [
            WorldAssetRoot(path)
        ]
    }
}

fn panel() -> impl Scene {
    bsn! {
        Node { position_type: PositionType::Absolute, top: px(16), left: px(16), padding: UiRect::all(px(16)) }
        BackgroundColor(Color::srgb(0.88, 0.81, 0.65))
        Children [
            Text("Earth Two: foundation scene\nThe Hull block from world/hull_block.json\nEsc: pause physics   Space: resume\nExisting assets; gameplay and shaders are still being ported")
            TextFont { font_size: px(18) }
            TextColor(Color::srgb(0.16, 0.12, 0.13))
        ]
    }
}

fn attach_probe_meshes(
    mut commands: Commands,
    ground: Query<Entity, Added<Ground>>,
    probes: Query<Entity, Added<Probe>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for entity in &ground {
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Cuboid::new(20.0, 1.0, 20.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.45, 0.24, 0.16))),
        ));
    }
    for entity in &probes {
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Sphere::new(0.5))),
            MeshMaterial3d(materials.add(Color::srgb(0.75, 0.62, 0.3))),
        ));
    }
}

/// Entities a layout spawns that the renderer's visibility must reach.
type NewlyPlaced = Or<(Added<LayoutRoot>, Added<Placed>, Added<Wheel>, Added<Lamp>)>;

/// Draws placed pieces: loads each model where the layout put it, and gives
/// the layout's hierarchy the visibility the renderer propagates.
fn attach_pieces(
    mut commands: Commands,
    assets: Res<AssetServer>,
    models: Query<(Entity, &PieceModel), Added<PieceModel>>,
    visible: Query<Entity, NewlyPlaced>,
) {
    for entity in &visible {
        commands.entity(entity).insert(Visibility::default());
    }
    for (entity, model) in &models {
        let scene: Handle<WorldAsset> = assets.load(format!("{}#Scene0", model.0));
        commands
            .entity(entity)
            .insert(Visibility::default())
            .with_child((Name::new("model"), WorldAssetRoot(scene)));
    }
}

fn attach_lamps(mut commands: Commands, lamps: Query<(Entity, &Lamp), Added<Lamp>>) {
    for (entity, lamp) in &lamps {
        let [r, g, b] = lamp.color;
        commands.entity(entity).insert(PointLight {
            color: Color::srgb(r, g, b),
            intensity: lamp.intensity * LAMP_LUMENS_PER_UNIT,
            range: lamp.range,
            ..default()
        });
    }
}

fn controls(keys: Res<ButtonInput<KeyCode>>, mut physics: ResMut<Time<Physics>>) {
    if keys.just_pressed(KeyCode::Escape) {
        crate::physics::set_paused(&mut physics, true);
    }
    if keys.just_pressed(KeyCode::Space) {
        crate::physics::set_paused(&mut physics, false);
    }
}
