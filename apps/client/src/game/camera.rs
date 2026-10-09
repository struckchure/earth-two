//! Orbit, chase, and collision arm from game/camera.go and game/drive.go.
use super::{Screen, seats::Seated};
use crate::{
    character::{self, Body, CharacterPhysics, Intent, Player},
    vehicle::Driving,
};
use bevy::prelude::*;
use std::f32::consts::PI;
#[derive(Component)]
pub struct GameCamera;
#[derive(Resource, Debug)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub still: f32,
    pub distance: f32,
    pub target: Option<Vec3>,
    pub title_time: f32,
    pub menu_fraction: f32,
    pub aspect: f32,
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 15f32.to_radians(),
            still: 0.,
            distance: 4.6,
            target: None,
            title_time: 0.0,
            menu_fraction: 0.0,
            aspect: 16.0 / 9.0,
        }
    }
}
pub fn wrap(a: f32) -> f32 {
    PI - (PI - a).rem_euclid(2. * PI)
}
impl Orbit {
    pub fn turn(&mut self, delta: Vec2) {
        if delta != Vec2::ZERO {
            self.yaw = wrap(self.yaw - delta.x * 0.004);
            self.pitch =
                (self.pitch + delta.y * 0.004).clamp(-25f32.to_radians(), 65f32.to_radians());
            self.still = 0.;
        }
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0., -self.yaw.cos())
    }
    pub fn offset(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        ) * self.distance
    }
    pub fn recenter(&mut self, facing: f32, moving: bool, dt: f32, driving: bool) {
        self.still += dt;
        let behind = wrap(facing + PI);
        if !moving
            || self.still < if driving { 0.6 } else { 1.5 }
            || (!driving && wrap(behind - self.yaw).abs() > 100f32.to_radians())
        {
            return;
        }
        let k = 1. - (-if driving { 3. } else { 2. } * dt).exp();
        self.yaw = wrap(self.yaw + wrap(behind - self.yaw) * k);
        self.pitch += (if driving {
            12f32.to_radians()
        } else {
            15f32.to_radians()
        } - self.pitch)
            * k;
    }
}
#[allow(clippy::type_complexity)]
pub fn steer(
    mut orbit: ResMut<Orbit>,
    mut view: ResMut<character::View>,
    screen: Res<State<Screen>>,
    driving: Res<Driving>,
    people: Query<(&Intent, &Children), (With<Player>, Without<Seated>)>,
    bodies: Query<&Transform, With<Body>>,
    time: Res<Time>,
) {
    if *screen.get() == Screen::Playing {
        if driving.active() {
            let f = driving.pose.1 * Vec3::Z;
            orbit.recenter(f.x.atan2(f.z), driving.speed > 1.5, time.delta_secs(), true);
        } else if let Ok((intent, children)) = people.single()
            && let Some(tr) = children.iter().find_map(|e| bodies.get(e).ok())
        {
            let f = tr.rotation * Vec3::Z;
            orbit.recenter(
                f.x.atan2(f.z),
                intent.move_dir != Vec3::ZERO,
                time.delta_secs(),
                false,
            );
        }
    }
    orbit.distance = if driving.active() {
        driving.camera
    } else {
        4.6
    };
    view.forward = orbit.forward();
}
fn arm(physics: &CharacterPhysics, pivot: Vec3, eye: Vec3, exclude: Entity) -> Vec3 {
    let delta = eye - pivot;
    let length = delta.length();
    if length < 0.001 {
        return eye;
    }
    physics
        .sweep_capsule_excluding(pivot, delta, 0.2, 0.4, exclude)
        .map_or(eye, |hit| {
            pivot + delta * ((hit.distance - 0.05).max(0.) / length)
        })
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn follow(
    mut orbit: ResMut<Orbit>,
    mut cameras: Query<&mut Transform, (With<GameCamera>, Without<Player>)>,
    people: Query<
        (
            Entity,
            &Transform,
            Option<&crate::presentation::MotionSamples>,
        ),
        With<Player>,
    >,
    driving: Res<Driving>,
    physics: CharacterPhysics,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    menu: Res<super::menu::Menu>,
) {
    let Ok((player, tr, samples)) = people.single() else {
        return;
    };
    let centre = samples.map_or(tr.translation, |s| {
        s.position(tr.translation, fixed.overstep_fraction())
    });
    let on_title = menu.on_title();
    if on_title {
        orbit.title_time += time.delta_secs();
    }
    let (offset, look, lag) = if *screen.get() == Screen::Dressing {
        (Vec3::new(0., 0.25, 3.1), Vec3::Y * 0.05, 5.0)
    } else if on_title {
        let a = 0.6 * (orbit.title_time * 0.2).sin();
        (
            Vec3::new(3.4 * a.sin(), 0.5, 3.4 * a.cos()),
            Vec3::Y * 0.15,
            5.0,
        )
    } else {
        (Vec3::Y * 0.7 + orbit.offset(), Vec3::Y * 0.7, 12.0)
    };
    let mut eye = centre + offset;
    let mut target = centre + look;
    if *screen.get() != Screen::Playing {
        let forward = target - eye;
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let shift = right
            * (-orbit.menu_fraction * forward.length() * 22.5f32.to_radians().tan() * orbit.aspect);
        eye += shift;
        target += shift;
    }
    let k = 1.0 - (-lag * time.delta_secs()).exp();
    let first = orbit.target.is_none();
    let target = orbit.target.map_or(target, |old| old.lerp(target, k));
    orbit.target = Some(target);
    for mut camera in &mut cameras {
        let mut eye = if first {
            eye
        } else {
            camera.translation.lerp(eye, k)
        };
        let mut look = target;
        if *screen.get() == Screen::Playing {
            let exclude = driving.vehicle.unwrap_or(player);
            look = arm(&physics, centre, target, exclude);
            eye = arm(&physics, look, eye, exclude);
            eye.y = eye.y.max(
                earth_two_world::terrain::ground_height(eye.x, eye.z)
                    + earth_two_world::terrain::GROUND_LEVEL
                    + 1.2,
            );
        }
        *camera = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    }
}
