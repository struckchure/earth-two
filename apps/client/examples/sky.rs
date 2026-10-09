//! The sky on its own: the dome, the sun, the planets, the stars and the
//! weather over a flat stretch of the Red, captured to a file.
//!
//! `cargo run --example sky` captures `build/rust/sky-11.png` (day) and
//! `build/rust/sky-22.png` (night) by running itself once for each hour.
//! With EARTH_TWO_HOUR set it captures that hour to `build/rust/sky.png`
//! (or EARTH_TWO_CAPTURE); EARTH_TWO_STORM holds the weather, and
//! EARTH_TWO_LOOK turns the camera (degrees from east through south;
//! default: towards the sun).

use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use earth_two_client::sky::{self, Daylight, SkyPlugin};

#[derive(Resource)]
struct CapturePath(String);

fn main() {
    if std::env::var_os("EARTH_TWO_HOUR").is_none() {
        // Both captures, each in a run of its own: an app runs once.
        let exe = std::env::current_exe().expect("own path");
        for hour in ["11", "22"] {
            let path = format!("build/rust/sky-{hour}.png");
            let status = std::process::Command::new(&exe)
                .env("EARTH_TWO_HOUR", hour)
                .env("EARTH_TWO_CAPTURE", &path)
                .status()
                .expect("run the capture");
            assert!(status.success(), "capturing {path} failed");
            println!("captured {path}");
        }
        return;
    }
    let path = std::env::var("EARTH_TWO_CAPTURE").unwrap_or_else(|_| "build/rust/sky.png".into());
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Earth Two — sky".into(),
                resolution: WindowResolution::new(1280, 720),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SkyPlugin)
        .insert_resource(CapturePath(path))
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

/// A camera at eye height on the flat Red, looking towards the sun (or
/// where EARTH_TWO_LOOK says), and the ground out to the horizon.
fn setup(
    mut commands: Commands,
    day: Res<Daylight>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let look = std::env::var("EARTH_TWO_LOOK")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|deg| {
            let a = deg.to_radians();
            Vec3::new(a.cos(), 0.0, a.sin())
        })
        .unwrap_or_else(|| Vec3::new(day.sun.x, 0.0, day.sun.z).normalize_or_zero());
    let eye = Vec3::new(0.0, 1.7, 0.0);
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            near: 0.1,
            far: sky::render::SKY_FAR,
            ..default()
        }),
        Transform::from_translation(eye).looking_at(eye + look + Vec3::Y * 0.12, Vec3::Y),
    ));
    // The Red's soil, flat to the horizon.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(16000.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(172, 96, 72),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, 0.0),
    ));
    // A few blocks, for scale and shadows.
    for (i, x) in [-6.0f32, 0.0, 7.0].iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 3.0 + i as f32, 2.0))),
            MeshMaterial3d(materials.add(Color::srgb_u8(120, 110, 104))),
            Transform::from_translation(
                eye + look * 18.0 + Vec3::new(*x, 0.0, 0.0) - Vec3::Y * 0.2,
            ),
        ));
    }
}

/// Captures the view once the sky's settled, and exits.
fn capture(
    mut commands: Commands,
    path: Res<CapturePath>,
    time: Res<Time<Real>>,
    mut requested: Local<bool>,
) {
    if *requested || time.elapsed_secs() < 2.5 {
        return;
    }
    *requested = true;
    info!("capturing the sky to {}", path.0);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.0.clone()))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}
