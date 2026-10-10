use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use std::time::Duration;

use crate::{
    scenes::{Probe, foundation_scene},
    world::WorldPlugin,
};

pub const FIXED_HZ: f64 = 60.0;

/// Avian allows manual steps while paused. Clear the previous delta so pausing
/// does not accidentally replay the last step before its clock resets.
pub fn set_paused(time: &mut Time<Physics>, paused: bool) {
    if paused {
        time.pause();
        time.advance_by(Duration::ZERO);
    } else {
        time.unpause();
    }
}

/// Keep the existing game's fixed cadence; rendering can run independently.
pub struct EarthPhysicsPlugin;

impl Plugin for EarthPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Go uses integer time.Second / 60 (16,666,666 ns). Rounding
        // to 16,666,667 ns changes f32 traversal release thresholds by a tick.
        app.insert_resource(Time::<Fixed>::from_duration(Duration::from_nanos(
            1_000_000_000 / 60,
        )))
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)))
        .add_systems(
            RunFixedMainLoop,
            hold_paused_bodies
                .after(avian3d::interpolation::TransformEasingSystems::Ease)
                .in_set(bevy::app::RunFixedMainLoopSystems::AfterFixedMainLoop),
        );
    }
}

/// Physics can pause independently of the fixed/render clocks. Show the last
/// simulated pose exactly instead of easing it with the still-running overstep.
#[allow(clippy::type_complexity)]
fn hold_paused_bodies(
    time: Res<Time<Physics>>,
    mut bodies: Query<(&Position, &Rotation, &mut Transform), (With<RigidBody>, Without<ChildOf>)>,
) {
    if !time.is_paused() {
        return;
    }
    for (position, rotation, mut transform) in &mut bodies {
        transform.translation = position.0;
        transform.rotation = rotation.0;
    }
}

/// The same BSN and physics scene used by the viewer, without a GPU or window.
pub fn smoke_app() -> App {
    headless_app(bevy::asset::AssetPlugin::default())
}

/// The repository's shared `assets/`, for headless tests of the real content.
pub fn source_assets() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string()
}

/// A windowless app with the foundation scene, physics at 60 Hz, stepped by
/// hand, and the world manifests loadable from `assets`.
pub fn headless_app(assets: bevy::asset::AssetPlugin) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        assets,
        bevy::scene::ScenePlugin,
        EarthPhysicsPlugin,
        WorldPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / FIXED_HZ,
    )))
    .add_systems(Startup, foundation_scene.spawn());
    app.finish();
    app.cleanup();
    app
}

pub fn probe_position(app: &mut App) -> Result<Vec3, String> {
    app.world_mut()
        .query_filtered::<&Position, With<Probe>>()
        .single(app.world())
        .map(|position| position.0)
        .map_err(|error| error.to_string())
}

/// Return an error so CI and the CLI fail if gravity/contact integration breaks.
pub fn run_smoke() -> Result<Vec3, String> {
    let mut app = smoke_app();
    for _ in 0..241 {
        app.update();
    }
    let position = probe_position(&mut app)?;
    if !position.is_finite() || (position.y - 0.5).abs() > 0.03 {
        return Err(format!("probe did not settle on the floor: {position:?}"));
    }
    Ok(position)
}
