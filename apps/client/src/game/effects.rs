//! Gameplay emission from cues.go/drivesound.go, using sky's effects.go port.
use crate::{
    character::Anim,
    landfall::{
        ground::{DOME_WALLS, rect_distance},
        surface::{Footing, Soundscape},
        zones::light_at,
    },
    sky::{Effects, Rgba},
    vehicle::{Spec, VehicleState},
};
use bevy::prelude::*;
/// The original sRGB zone fill (dust is unlit by the mesh material).
pub fn light(at: Vec3) -> Rgba {
    let [r, g, b, a] = light_at(at);
    Rgba { r, g, b, a }
}
pub fn foot(fx: &mut Effects, under: Footing, at: Vec3, velocity: Vec3, speed: f32) {
    if under.loose() && rect_distance(&DOME_WALLS, at.x, at.z) > 0. {
        fx.puff(at, velocity, speed, light(at));
    }
}
#[allow(clippy::too_many_arguments)]
pub fn body(
    fx: &mut Effects,
    feet: Vec3,
    velocity: Vec3,
    under: Footing,
    grounded: bool,
    anim: Anim,
    landing: Option<f32>,
    dt: f32,
) {
    if !under.loose() {
        return;
    }
    let light = light(feet + Vec3::Y);
    if let Some(k) = landing {
        fx.ring(feet, 8 + (16. * k) as usize, 0.6 + 0.8 * k, light);
    }
    if grounded && matches!(anim, Anim::Slide | Anim::Roll) {
        fx.trail(feet, velocity, dt, light);
    }
}
/// Fractional puffs are retained per wheel, including while its contact is lost.
/// The caller resets these when entering another vehicle, as Go does.
#[allow(clippy::too_many_arguments)]
pub fn wheels(
    fx: &mut Effects,
    dust: &mut Vec<f32>,
    spec: &Spec,
    state: &VehicleState,
    transform: &Transform,
    velocity: Vec3,
    scape: &Soundscape,
    dt: f32,
) {
    if dust.len() != state.wheels.len() {
        *dust = vec![0.; state.wheels.len()];
    }
    for (i, w) in state.wheels.iter().enumerate() {
        if !w.contact || i >= spec.wheels.len() {
            continue;
        }
        let radius = if spec.wheels[i].radius == 0. {
            1.
        } else {
            spec.wheels[i].radius
        };
        let at = transform.translation + transform.rotation * w.transform.translation
            - Vec3::Y * radius * 0.8;
        if !scape.surface_at(at, false).loose() {
            continue;
        }
        let k = (state.speed.abs().max((w.spin * radius).abs()) / 10.).clamp(0., 1.);
        if k < 0.1 {
            continue;
        }
        dust[i] += 18. * k * dt;
        while dust[i] >= 1. {
            dust[i] -= 1.;
            fx.wheel(at, velocity, k, light(at));
        }
    }
}
/// Called only for an accepted crash (shared sound threshold/cooldown).
pub fn crash(fx: &mut Effects, at: Vec3, normal: Vec3, into: f32, ground: bool) {
    let k = ((into - 2.5) / 12.).clamp(0., 1.);
    if ground {
        fx.ring(at, 10 + (20. * k) as usize, 0.8 + k, light(at));
    } else if into > 7. {
        fx.sparks(at, normal, 12 + (24. * k) as usize);
    }
}
