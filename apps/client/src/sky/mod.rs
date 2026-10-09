//! The sky and the atmosphere: the world's clock, the sun over the day,
//! the sky dome and the neighbour planets, the stars by night, the dust
//! storms and the haze, the fill and the sun's light they make, and the
//! dust and sparks the effects throw up. See README.md for what other
//! systems read.
//!
//! The clock, the sun's maths, the weather's schedule, the star catalogue
//! and the particles' motion are rendering-free; the dome, the stars' and
//! the particles' meshes and the blown dust are behind the `viewer`
//! feature (render.rs).

pub mod clock;
pub mod colour;
pub mod daylight;
pub mod effects;
pub mod skytex;
pub mod stars;
pub mod sun;
pub mod time;
pub mod weather;

#[cfg(feature = "viewer")]
pub mod particles;
#[cfg(feature = "viewer")]
pub mod render;

pub use clock::{Clock, DAYLIGHT_HOURS, day_strip, part_of_day};
pub use colour::{Rgba, mix_colour, rgb};
pub use daylight::{Daylight, SkyPaint, Sun};
pub use effects::{Effects, Particle};
pub use time::UnixTime;
pub use weather::{Ambient, Haze, SunLight, Weather, conditions, storm_at};

use bevy::prelude::*;

/// Where the sky's systems run in `Update`: the sun turned, then the
/// weather on it. Particle motion is in PostUpdate after gameplay emissions.
/// Systems that read `Daylight`,
/// `Weather`, `Haze`, `Ambient` or `SunLight` go after this set.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkySystems;

/// The sky: the sun where the clock has it (daylight.rs), the weather on
/// it (weather.rs) and the dust in the air (effects.rs); and, with the
/// viewer, the dome, the stars and what they're drawn with (render.rs).
pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        let clock = Clock::from_env();
        let day = Daylight::new(&clock);
        app.insert_resource(clock)
            .insert_resource(day)
            .insert_resource(Weather::from_env())
            .init_resource::<Haze>()
            .init_resource::<Ambient>()
            .init_resource::<SunLight>()
            .init_resource::<Effects>()
            .add_systems(Startup, daylight::spawn_sun)
            .add_systems(
                Update,
                (daylight::turn_sun, weather::blow)
                    .chain()
                    .in_set(SkySystems),
            )
            .add_systems(
                PostUpdate,
                effects::move_effects
                    .after(TransformSystems::Propagate)
                    .in_set(effects::EffectsSystems),
            );
        #[cfg(feature = "viewer")]
        app.add_plugins(render::SkyRenderPlugin);
    }
}
