//! The time of day, on the real clock as the world runs. The sun rises
//! about 05:00, crosses the south low at noon and sets about 19:00
//! (sun.rs). As it nears the horizon the light goes the way
//! docs/look-and-feel.md has it at dusk: the sky violet overhead and rose
//! at the horizon, the sun deep orange and dimmer, the shadows long; and
//! dawn the same. At night the sky's dark and starred, the only light
//! outside a faint cold one off the neighbour planets, and the lamps and
//! beacons light the town. The weather's storms blow on top of whatever
//! the hour is.

use super::{
    clock::Clock,
    stars::sky_turn,
    sun::{dusk_at, light_from, night_at, sun_at},
    time::UnixTime,
};
use bevy::prelude::*;
use std::f64::consts::PI;

/// A resource: when the sun's at, the way to it, the way the scene's light
/// comes from, how much of dusk's colour there is and how far into night
/// it is, how far the stars' sky has turned, and the sun, dusk and night
/// the sky was last painted for. Other systems read it; `turn_sun` writes
/// it each frame.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Daylight {
    /// When the sun's at: the clock's time, or the held hour today.
    pub now: UnixTime,
    /// The way to the sun (Go's `sunFrom`), a unit direction.
    pub sun: Vec3,
    /// The way the scene's light comes from: the sun's, or the planets' by
    /// night, never from under the ground.
    pub light: Vec3,
    /// How much of dusk's colour there is, 0 to 1.
    pub dusk: f32,
    /// How far into night it is, 0 to 1.
    pub night: f32,
    /// How far the stars' sky has turned about its pole, radians.
    pub turn: f32,
    /// What the sky's texture was last painted for.
    pub baked: SkyPaint,
}

/// What the sky's painted for: the sun's way, dusk, night, and how far the
/// stars' sky has turned.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkyPaint {
    pub dir: Vec3,
    pub dusk: f32,
    pub night: f32,
    pub turn: f32,
    pub done: bool,
}

/// How far the sun moves, or the stars' sky turns (radians), or dusk or
/// night comes on, before the sky's painted again: the glare round the sun
/// and the Milky Way are in its texture.
pub const SKY_REPAINT: f64 = 0.4 * PI / 180.0;
pub const DUSK_REPAINT: f32 = 0.03;

impl SkyPaint {
    /// Whether the sky painted for `self` wants painting again for `now`.
    pub fn stale(&self, now: &SkyPaint) -> bool {
        if !self.done {
            return true;
        }
        if (self.dusk - now.dusk).abs() >= DUSK_REPAINT
            || (self.night - now.night).abs() >= DUSK_REPAINT
        {
            return true;
        }
        // Once the sun's well down its glare's gone, so its moving doesn't
        // show; the Milky Way's turning does.
        if now.night < 0.99 && f64::from(self.dir.dot(now.dir)).min(1.0).acos() > SKY_REPAINT {
            return true;
        }
        now.night > 0.01 && f64::from((self.turn - now.turn).abs()) > SKY_REPAINT
    }
}

impl Daylight {
    /// The daylight as the clock has it now.
    pub fn new(clock: &Clock) -> Self {
        Self::at(clock.now())
    }

    /// The daylight with the sun where it is at `now`.
    pub fn at(now: UnixTime) -> Self {
        let sun = sun_at(now);
        let (dusk, night) = (dusk_at(sun), night_at(sun));
        Self {
            now,
            sun,
            light: light_from(sun, night),
            dusk,
            night,
            turn: sky_turn(now),
            baked: SkyPaint::default(),
        }
    }

    /// What the sky wants painting for now.
    pub fn paint(&self) -> SkyPaint {
        SkyPaint {
            dir: self.sun,
            dusk: self.dusk,
            night: self.night,
            turn: self.turn,
            done: true,
        }
    }

    /// Whether the sky's texture is out of date for now.
    pub fn wants_repaint(&self) -> bool {
        self.baked.stale(&self.paint())
    }

    /// Whether the lamps, beacons and headlamps are lit: by night, and as
    /// dusk's well on (what Go sets on `vehicle.LightCycle.Night`).
    pub fn lamps_on(&self) -> bool {
        self.night > 0.01 || self.dusk > 0.75
    }
}

/// The sun's light: the directional light the sky turns to follow the sun.
#[derive(Component, Debug, Default)]
pub struct Sun;

/// Puts the sun where it is now: the light's way, and the daylight's dusk
/// and night, that the weather, the sky and the stars follow.
pub fn turn_sun(
    clock: Res<Clock>,
    mut day: ResMut<Daylight>,
    mut suns: Query<&mut Transform, With<Sun>>,
) {
    let fresh = Daylight::at(clock.now());
    let baked = day.baked;
    *day = Daylight { baked, ..fresh };
    for mut tr in &mut suns {
        let at = tr.translation;
        *tr = Transform::from_translation(at).looking_at(at - day.light, Vec3::Y);
    }
}

/// The sun's light, where the Go client spawns it: looking the way the
/// light goes.
pub fn spawn_sun(mut commands: Commands, day: Res<Daylight>) {
    commands.spawn((
        Name::new("sun"),
        Sun,
        Transform::IDENTITY.looking_at(-day.light, Vec3::Y),
    ));
}
