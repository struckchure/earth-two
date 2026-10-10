//! Read-only review tour using the supported subset of Go game.View JSON.
//! Orbit shots exercise the real camera; fixed shots isolate presentation.
use bevy::{
    input::InputSystems,
    prelude::*,
    transform::TransformSystems,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowResolution},
};
use earth_two_client::{
    character::{CharacterController, Intent},
    game::{
        GamePlugin, Screen, Session, StartAt,
        camera::{GameCamera, Orbit},
        menu::Menu,
    },
    presentation::MotionSamples,
};
use serde::Deserialize;
use std::{path::PathBuf, time::Instant};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct View {
    name: String,
    player: [f32; 3],
    #[serde(default)]
    ground: bool,
    yaw: f32,
    pitch: f32,
    eye: Option<[f32; 3]>,
    target: Option<[f32; 3]>,
}
#[derive(Resource)]
struct Tour {
    views: Vec<View>,
    out: PathBuf,
    at: usize,
    frame: usize,
    last_frame: Option<Instant>,
    frame_times_ms: Vec<f64>,
    cloth: bool,
}
fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("views.json");
    let out = PathBuf::from(args.next().expect("output directory"));
    let cloth = match args.next().as_deref() {
        None | Some("--cloth=on") => true,
        Some("--cloth=off") => false,
        Some(other) => panic!("unknown option {other}; expected --cloth=on or --cloth=off"),
    };
    assert!(!out.exists(), "preserve existing evidence");
    std::fs::create_dir_all(&out).unwrap();
    let views: Vec<View> = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    assert!(!views.is_empty());
    unsafe {
        std::env::set_var("EARTH_TWO_IDENTITY_PATH", out.join("disposable-profile/pk"));
    }
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: earth_two_client::physics::source_assets(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Rust parity tour".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(StartAt::Hull)
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::presentation::ClothEnabled(cloth))
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .insert_resource(Tour {
            views,
            out,
            at: 0,
            frame: 0,
            last_frame: None,
            frame_times_ms: Vec::with_capacity(60),
            cloth,
        })
        .add_systems(PreUpdate, prepare.after(InputSystems))
        .add_systems(PostUpdate, shot.before(TransformSystems::Propagate))
        .run();
}
fn prepare(w: &mut World) {
    if !w.resource::<Session>().ready || *w.resource::<State<Screen>>().get() == Screen::Loading {
        return;
    }
    let mut tour = w.resource_mut::<Tour>();
    if tour.at >= tour.views.len() {
        return;
    }
    let now = Instant::now();
    if let Some(last) = tour.last_frame.replace(now)
        && (90..=149).contains(&tour.frame)
    {
        tour.frame_times_ms
            .push(now.duration_since(last).as_secs_f64() * 1000.0);
    }
    tour.frame += 1;
    let v = &tour.views[tour.at];
    let (frame, yaw, pitch, mut pos, ground) =
        (tour.frame, v.yaw, v.pitch, Vec3::from(v.player), v.ground);
    if ground {
        pos.y = earth_two_world::terrain::ground_height(pos.x, pos.z)
            + earth_two_world::terrain::GROUND_LEVEL
            + 1.;
    }
    w.resource_mut::<Menu>().stack.clear();
    w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    let p = w.resource::<Session>().player.unwrap();
    if frame <= 40 {
        w.get_mut::<Transform>(p).unwrap().translation = pos;
        w.get_mut::<CharacterController>(p).unwrap().velocity = Vec3::ZERO;
        *w.get_mut::<MotionSamples>(p).unwrap() = MotionSamples::at(pos, 1.8);
    }
    *w.get_mut::<Intent>(p).unwrap() = default();
    let mut o = w.resource_mut::<Orbit>();
    o.yaw = yaw;
    o.pitch = pitch;
    o.still = 0.;
    for (mut win, mut cursor) in w
        .query_filtered::<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>()
        .iter_mut(w)
    {
        win.focused = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}
fn shot(w: &mut World) {
    let t = w.resource::<Tour>();
    if t.at >= t.views.len() {
        w.write_message(AppExit::Success);
        return;
    }
    if t.frame == 0 {
        return;
    }
    let v = &t.views[t.at];
    let (frame, eye, target, name, out) = (t.frame, v.eye, v.target, v.name.clone(), t.out.clone());
    if let (Some(eye), Some(target)) = (eye, target) {
        for mut tr in w
            .query_filtered::<&mut Transform, With<GameCamera>>()
            .iter_mut(w)
        {
            *tr = Transform::from_translation(Vec3::from(eye))
                .looking_at(Vec3::from(target), Vec3::Y);
        }
    }
    if frame == 150 {
        let mut samples = w.resource::<Tour>().frame_times_ms.clone();
        samples.sort_by(f64::total_cmp);
        if !samples.is_empty() {
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let percentile = |p: f64| {
                samples[((samples.len() as f64 * p).ceil() as usize)
                    .saturating_sub(1)
                    .min(samples.len() - 1)]
            };
            let stats = serde_json::json!({
                "frames": samples.len(),
                "mean_ms": mean,
                "median_ms": percentile(0.50),
                "p95_ms": percentile(0.95),
                "p99_ms": percentile(0.99),
                "fps_from_mean": 1000.0 / mean,
                "cloth": w.resource::<Tour>().cloth,
                "cloth_iterations": earth_two_client::presentation::verlet::CLOTH_ITERATIONS,
                "frame_times_ms": samples.clone(),
                "method": "monotonic wall time between native app updates; settled frames 90-149"
            });
            println!(
                "tour {name}: {:.1} FPS, mean {:.2} ms, median {:.2} ms, p95 {:.2} ms, p99 {:.2} ms ({} frames)",
                1000.0 / mean,
                mean,
                percentile(0.50),
                percentile(0.95),
                percentile(0.99),
                w.resource::<Tour>().frame_times_ms.len()
            );
            std::fs::write(
                out.join(format!("{name}.perf.json")),
                serde_json::to_vec_pretty(&stats).unwrap(),
            )
            .unwrap();
        }
        let camera = w
            .query_filtered::<&Transform, With<GameCamera>>()
            .single(w)
            .unwrap();
        let eye = camera.translation.to_array();
        let forward = camera.forward().to_array();
        let p = w.resource::<Session>().player.unwrap();
        let centre = w.get::<Transform>(p).unwrap().translation.to_array();
        let grounded = w.get::<CharacterController>(p).unwrap().grounded;
        let window = w
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(w)
            .unwrap();
        let metadata = serde_json::json!({"eye":eye,"forward":forward,"player_centre":centre,"grounded":grounded,"physical_size":[window.physical_width(),window.physical_height()],"hour":11,"storm":0,"frames":150,"cloth":w.resource::<Tour>().cloth,"scope":"Real orbit unless explicit eye/target. Display-paced; animation phase is not synchronized with Go."});
        std::fs::write(
            out.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .unwrap();
        use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
        w.spawn(Screenshot::primary_window())
            .observe(save_to_disk(out.join(format!("{name}.png"))));
    }
    // Allow screenshot readback to finish before switching view or exiting.
    if frame >= 160 {
        let mut t = w.resource_mut::<Tour>();
        t.at += 1;
        t.frame = 0;
        t.last_frame = None;
        t.frame_times_ms.clear();
    }
}
