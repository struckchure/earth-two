//! Headlamps: lamps mounted on the bodywork, automatic with the night while
//! someone's driving, or switched with H. Port of `vehicle/headlamps.go`.
//! The beam is data ([`HeadlampBeam`], what `shading.SpotLight` carried);
//! the viewer gives it a light.

use bevy::prelude::*;

use super::{
    Drivable,
    drive::Controls,
    drive::Driving,
    spec::{Spec, headlamp_mounts},
};
use crate::character;

/// Lets the game tell vehicles when it is dark.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LightCycle {
    pub night: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HeadlampMode {
    #[default]
    Automatic,
    Off,
    On,
}

/// Marks a lamp mounted on the vehicle's root.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Headlamp {
    pub radius: f32,
}

/// The lamp's beam, as the shading port's spot light has it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct HeadlampBeam {
    pub color: [u8; 4],
    pub intensity: f32,
    pub range: f32,
    pub inner: f32, // degrees
    pub outer: f32, // degrees
    pub enabled: bool,
    pub priority: i32,
}

pub const HEADLAMP_COLOR: [u8; 4] = [255, 240, 204, 255];

pub fn spawn_headlamps(c: &mut ChildSpawnerCommands, spec: &Spec) {
    for mount in headlamp_mounts(spec) {
        let at = Vec3::from(mount.at);
        // Forward is -Z in a Transform, but the vehicle's nose is +Z.
        let pose =
            Transform::from_translation(at).looking_at(at + Vec3::new(0.0, -0.10, 1.0), Vec3::Y);
        c.spawn((
            Name::new("headlamp"),
            Headlamp {
                radius: mount.radius,
            },
            pose,
            HeadlampBeam {
                color: HEADLAMP_COLOR,
                intensity: 5.0,
                range: 55.0,
                inner: 16.0,
                outer: 28.0,
                enabled: false,
                priority: 0,
            },
        ));
    }
}

/// Powers each vehicle's lamps: automatic ones with the night and the
/// driver, manual ones as H left them; the driven vehicle's beams get
/// priority.
pub fn update_headlamps(
    mut cars: Query<(Entity, &mut Drivable, Option<&mut Controls>)>,
    mut lamps: Query<(&mut HeadlampBeam, &ChildOf), With<Headlamp>>,
    cycle: Res<LightCycle>,
    controls: Res<character::Controls>,
    mut driving: ResMut<Driving>,
) {
    let night = cycle.night;
    for (e, mut d, keys) in &mut cars {
        // A manual off applies to this drive. Explicit on remains powered
        // after leaving, until the player switches it off again.
        if d.driver.is_none() && d.lights_mode == HeadlampMode::Off {
            d.lights_mode = HeadlampMode::Automatic;
        }
        d.headlamps = d.lights_mode == HeadlampMode::On
            || (d.lights_mode == HeadlampMode::Automatic && night && d.driver.is_some());
        // H is this frame's press, taken whether or not it's acted on.
        let pressed = keys.is_some_and(|mut keys| std::mem::take(&mut keys.headlamps));
        if Some(e) == driving.vehicle {
            if pressed && controls.enabled {
                d.headlamps = !d.headlamps;
                d.lights_mode = if d.headlamps {
                    HeadlampMode::On
                } else {
                    HeadlampMode::Off
                };
            }
            driving.headlamps = d.headlamps;
        }
    }
    for (mut beam, parent) in &mut lamps {
        beam.enabled = false;
        beam.priority = 0;
        if let Ok((root, d, _)) = cars.get(parent.parent()) {
            beam.enabled = d.headlamps;
            if Some(root) == driving.vehicle {
                beam.priority = 1;
            }
        }
    }
}

/// Gives each lamp a spot light, and switches it with its beam. How bright
/// and how far is the shading port's to match; this keeps the beams
/// following the bodywork.
#[cfg(feature = "viewer")]
pub fn light_headlamps(
    mut commands: Commands,
    lamps: Query<(Entity, &HeadlampBeam, Option<&SpotLight>), With<Headlamp>>,
) {
    for (e, beam, light) in &lamps {
        let [r, g, b, _] = beam.color;
        let intensity = if beam.enabled {
            beam.intensity * 100_000.0
        } else {
            0.0
        };
        if light.is_none_or(|l| l.intensity != intensity) {
            commands.entity(e).insert(SpotLight {
                color: Color::srgb_u8(r, g, b),
                intensity,
                range: beam.range,
                inner_angle: beam.inner.to_radians(),
                outer_angle: beam.outer.to_radians(),
                shadow_maps_enabled: false,
                ..default()
            });
        }
    }
}
