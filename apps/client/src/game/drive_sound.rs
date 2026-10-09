//! Engine, tyre, entry, skid and collision cues from game/drivesound.go.
use super::{
    Screen,
    camera::GameCamera,
    cues::{Cue, voice},
    sound::LoopState,
};
use crate::{
    landfall::{surface::Soundscape, terrain::TerrainBody},
    vehicle::{Drivable, Driving, SeatAnim, VehicleInput, VehicleState},
};
use avian3d::prelude::*;
use bevy::prelude::*;
pub const NAMES: [&str; 6] = [
    "engine_bike",
    "engine_trike",
    "engine_buggy",
    "engine_rover",
    "engine_truck",
    "tyres",
];
#[derive(Clone, Copy, Debug)]
pub struct EngineSound {
    pub index: usize,
    pub pitch: f32,
    pub rev: f32,
    pub idle: f32,
    pub throttling: f32,
}
pub fn engine_sound(kind: &str) -> EngineSound {
    let (index, pitch, rev, idle, throttling) = match kind {
        "bike" => (0, 0.9, 2.1, 0.4, 0.55),
        "trike" => (1, 0.85, 1.9, 0.45, 0.5),
        "rover" => (3, 0.6, 1.5, 0.5, 0.35),
        "truck" => (4, 0.75, 1.45, 0.6, 0.4),
        _ => (2, 0.8, 1.8, 0.45, 0.5),
    };
    EngineSound {
        index,
        pitch,
        rev,
        idle,
        throttling,
    }
}
#[derive(Resource, Default)]
pub struct DriveSound {
    pub loops: [LoopState; 6],
    pub vehicle: Option<Entity>,
    pub gear: i32,
    pub shift: f32,
    pub skid: f32,
    pub crash: f32,
}
impl DriveSound {
    pub fn tick(&mut self, dt: f32) {
        self.shift = (self.shift - dt).max(0.);
        self.skid = (self.skid - dt).max(0.);
        self.crash = (self.crash - dt).max(0.);
    }
    pub fn quiet(&mut self, dt: f32) {
        for s in &mut self.loops {
            s.set(0., 0., 0., dt);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn engine(
        &mut self,
        kind: &str,
        min: f32,
        max: f32,
        state: &VehicleState,
        input: &VehicleInput,
        loose: bool,
        playing: bool,
        dt: f32,
    ) {
        if state.gear != self.gear {
            if state.gear != 0 && self.gear != 0 {
                self.shift = 0.18;
            }
            self.gear = state.gear;
        }
        let sound = engine_sound(kind);
        let rev = ((state.rpm - min) / (max - min).max(1.)).clamp(0., 1.);
        let pitch = sound.pitch * (1. + (sound.rev - 1.) * rev) * (1. - 0.12 * self.shift / 0.18);
        let volume = if playing {
            (sound.idle + sound.throttling * input.forward.abs().clamp(0., 1.) + 0.1 * rev) * 0.8
        } else {
            0.
        };
        for (i, s) in self.loops[..5].iter_mut().enumerate() {
            s.set(if i == sound.index { volume } else { 0. }, pitch, 0., dt);
        }
        let speed = state.speed.abs();
        let touching = state.touching as f32 / state.wheels.len().max(1) as f32;
        let roll = if playing {
            (speed / 18.).clamp(0., 1.) * touching * 0.8 * if loose { 1. } else { 0.5 }
        } else {
            0.
        };
        self.loops[5].set(roll * 0.8, 0.8 + 0.5 * (speed / 25.).clamp(0., 1.), 0., dt);
    }
    pub fn skid_volume(&mut self, slip: f32, speed: f32, touching: f32, brake: f32) -> Option<f32> {
        if self.skid == 0. && touching > 0.4 && (slip > 3.5 || (brake > 0.5 && speed > 6.)) {
            self.skid = 1.2;
            Some(0.5 + 0.4 * (slip / 10.).clamp(0., 1.))
        } else {
            None
        }
    }
    pub fn crash_sound(&mut self, into: f32, ground: bool) -> Option<(&'static str, f32, f32)> {
        if self.crash > 0. || into < 2.5 {
            return None;
        }
        self.crash = 0.3;
        let k = ((into - 2.5) / 12.).clamp(0., 1.);
        Some((
            if ground {
                "crash_ground"
            } else {
                "crash_metal"
            },
            0.5 + 0.5 * k,
            if ground { 1. } else { 1. - 0.15 * k },
        ))
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn drive_cues(
    mut memory: ResMut<DriveSound>,
    driving: Res<Driving>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    cameras: Query<&Transform, With<GameCamera>>,
    cars: Query<(
        &Drivable,
        &VehicleInput,
        &VehicleState,
        &Transform,
        &LinearVelocity,
    )>,
    scape: Option<Res<Soundscape>>,
    mut cues: MessageWriter<Cue>,
) {
    let dt = time.delta_secs();
    memory.tick(dt);
    let (Ok(camera), Some(scape)) = (cameras.single(), scape) else {
        memory.quiet(dt);
        return;
    };
    let v = voice(camera, *screen.get());
    if driving.vehicle != memory.vehicle {
        if let Some(e) = driving.vehicle.or(memory.vehicle)
            && let Ok((car, _, _, tr, _)) = cars.get(e)
        {
            let (name, vol) = if car.handling.seat == SeatAnim::Drive {
                ("door", 0.9)
            } else {
                ("cloth", 0.7)
            };
            cues.write(v.at(name, tr.translation, vol, 4., 30., 1.));
        }
        memory.vehicle = driving.vehicle;
        memory.gear = driving.gear;
    }
    let Some(e) = driving.vehicle else {
        memory.quiet(dt);
        return;
    };
    let Ok((car, input, state, tr, vel)) = cars.get(e) else {
        memory.quiet(dt);
        return;
    };
    memory.engine(
        &car.spec.handling,
        car.handling.min_rpm,
        car.handling.max_rpm,
        state,
        input,
        scape.surface_at(tr.translation, false).loose(),
        *screen.get() == Screen::Playing,
        dt,
    );
    let slip = vel.dot(*tr.right()).abs();
    let touching = state.touching as f32 / state.wheels.len().max(1) as f32;
    if let Some(volume) = memory.skid_volume(slip, state.speed.abs(), touching, input.hand_brake) {
        cues.write(v.at("skid", tr.translation, volume, 3., 40., 1.));
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn crash_cues(
    mut events: MessageReader<CollisionStart>,
    collisions: Collisions,
    driving: Res<Driving>,
    velocities: Query<&LinearVelocity>,
    ground: Query<(), With<TerrainBody>>,
    cameras: Query<&Transform, With<GameCamera>>,
    screen: Res<State<Screen>>,
    mut memory: ResMut<DriveSound>,
    mut cues: MessageWriter<Cue>,
) {
    let (Some(vehicle), Ok(camera)) = (driving.vehicle, cameras.single()) else {
        events.clear();
        return;
    };
    let Ok(velocity) = velocities.get(vehicle) else {
        events.clear();
        return;
    };
    let v = voice(camera, *screen.get());
    for hit in events.read() {
        let a = hit.body1.unwrap_or(hit.collider1);
        let b = hit.body2.unwrap_or(hit.collider2);
        let other = if a == vehicle {
            b
        } else if b == vehicle {
            a
        } else {
            continue;
        };
        let Some(pair) = collisions.get(hit.collider1, hit.collider2) else {
            continue;
        };
        let Some(manifold) = pair.manifolds.first() else {
            continue;
        };
        let Some(point) = manifold.points.first() else {
            continue;
        };
        if let Some((name, volume, pitch)) =
            memory.crash_sound(velocity.dot(manifold.normal).abs(), ground.contains(other))
        {
            cues.write(v.at(name, point.point, volume, 4., 60., pitch));
        }
    }
}
