//! The game's look: toon shading for everything, and ink outlines for what
//! asks for them. Ported from the Go `shading` package.
//!
//! This file is the rendering-free part: the look's settings, the light
//! zones, the haze, the lamp and headlamp maths, and the shadow control.
//! The materials, shaders and the systems that feed them are in
//! [`plugin`], behind the `viewer` feature.

use bevy::prelude::*;

#[cfg(feature = "viewer")]
pub mod plugin;
#[cfg(feature = "viewer")]
pub use plugin::{
    Daylight, OutlineMaterial, Outlined, Painted, PaintedMaterial, Smooth, lamp_point_light,
    sun_cascades,
};

/// An 8-bit colour as the Go code gives it (`color.RGBA` without alpha).
pub type Rgb = [u8; 3];

/// A colour's channels as the shaders take them, 0 to 1.
pub fn rgb(c: Rgb) -> Vec3 {
    Vec3::new(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
    )
}

/// `c` at brightness `b`.
pub fn scaled(c: Rgb, b: f32) -> Vec3 {
    rgb(c) * b
}

/// How many zones the toon shader takes.
pub const MAX_ZONES: usize = 4;

/// How many headlamps light the scene at once.
pub const MAX_SPOTS: usize = 8;

/// How far away, in metres, outlines start thinning with distance rather
/// than swallowing what they're around.
pub const OUTLINE_REACH: f32 = 7.0;

/// The look: the Go `shading.Plugin`'s settings.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Look {
    /// Tints the ambient light on shaded faces. Cast shadows darken this
    /// fill towards black.
    pub shadow_color: Rgb,
    /// How wide the edges between the bands are, in the cosine of the
    /// angle to the light.
    pub softness: f32,
    /// Where the half-lit band ends.
    pub mid_band: f32,
    /// The light on the edges of what's lit; a higher power and threshold
    /// keep it nearer the edge. Black for none.
    pub rim_color: Rgb,
    pub rim_power: f32,
    pub rim_threshold: f32,
    /// Tints an outline's surface; the width is in pixels.
    pub outline_color: Rgb,
    pub outline_width: f32,
    /// Haze: what's far off fades to `fog_color`, a share 1 - 1/e of the
    /// way at `fog_distance` (0 for no haze), and more with distance, up to
    /// `fog_end`, past which (the sky) none.
    pub fog_color: Rgb,
    pub fog_distance: f32,
    pub fog_end: f32,
    /// The light bounced up off the ground, at `ground_brightness`: what
    /// faces down gets it in place of the ambient light, which then comes
    /// from the sky above. Black for an ambient the same from everywhere.
    pub ground_fill: Rgb,
    pub ground_brightness: f32,
    /// Scales the light (0 for 1), and colours brighter than `knee` roll
    /// off towards white rather than clipping, keeping their hue (0 for
    /// none).
    pub exposure: f32,
    pub knee: f32,
    /// Places lit differently from the open air, at most [`MAX_ZONES`] of
    /// them; where they overlap, the later wins.
    pub zones: Vec<Zone>,
}

impl Default for Look {
    /// The Go zero value: no haze, no rim, no fill, exposure 1.
    fn default() -> Self {
        Self {
            shadow_color: [0, 0, 0],
            softness: 0.0,
            mid_band: 0.0,
            rim_color: [0, 0, 0],
            rim_power: 0.0,
            rim_threshold: 0.0,
            outline_color: [0, 0, 0],
            outline_width: 0.0,
            fog_color: [0, 0, 0],
            fog_distance: 0.0,
            fog_end: 0.0,
            ground_fill: [0, 0, 0],
            ground_brightness: 0.0,
            exposure: 0.0,
            knee: 0.0,
            zones: Vec::new(),
        }
    }
}

impl Look {
    /// The game's look, as `game/game.go` configures the Go plugin.
    pub fn earth_two() -> Self {
        Self {
            // Violet, but light enough that shaded faces still read.
            shadow_color: [205, 190, 235],
            // Wide enough that the bands blend like brushwork rather than
            // cut like a cel.
            softness: 0.09,
            // Low, so the ground under the low sun is in its full light.
            mid_band: 0.2,
            // No rim light: it follows the camera, so it reads as a light
            // carried round with the player.
            rim_color: [0, 0, 0],
            rim_power: 3.0,
            rim_threshold: 0.35,
            outline_color: [60, 40, 55],
            outline_width: 1.5,
            // Dust in the air: the Fringe hazes into the sky's horizon with
            // distance, so the seats loom out of it.
            fog_color: SKY_HORIZON,
            fog_distance: 1600.0,
            fog_end: BODY_DISTANCE - 200.0,
            // The red soil throws the sun back up under things.
            ground_fill: [205, 120, 86],
            ground_brightness: 0.24,
            exposure: 0.0,
            // The low sun is brighter than white on what faces it: roll it
            // off rather than clipping.
            knee: 0.8,
            zones: light_zones(),
        }
    }

    /// The exposure the shaders get: 0 means 1.
    pub fn exposure_or_one(&self) -> f32 {
        if self.exposure == 0.0 {
            1.0
        } else {
            self.exposure
        }
    }

    /// The haze as the look starts it.
    pub fn haze(&self) -> Haze {
        Haze {
            color: self.fog_color,
            distance: self.fog_distance,
            end: self.fog_end,
            veil: 0.0,
        }
    }

    /// The colour of the light at `p` for what's drawn unlit there (the
    /// dust): `sun` outside, the zones' fill inside them. `game.lightAt`.
    pub fn light_at(&self, sun: Rgb, p: Vec3) -> Rgb {
        let mut c = sun;
        for z in &self.zones {
            c = mix_colour(c, z.ambient, z.weight(p));
        }
        c
    }
}

/// A box of the world, `min` to `max`, lit its own way: inside it the
/// ambient light is `ambient` at `brightness`, and the sun is tinted `sun`
/// (white leaves it be, black puts it out), fading in over `blend` metres
/// from its faces. Shadows still fall in it, so a roof still keeps the sun
/// off what's under it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub min: Vec3,
    pub max: Vec3,
    pub blend: f32,
    pub ambient: Rgb,
    pub brightness: f32,
    pub sun: Rgb,
}

impl Zone {
    /// How far into the zone `p` is, 0 to 1: the toon shader's blend, so
    /// what's heard and what's lit change in the same place.
    pub fn weight(&self, p: Vec3) -> f32 {
        let inside = |lo: f32, hi: f32, v: f32| {
            smoothstep(lo, lo + self.blend, v) * (1.0 - smoothstep(hi - self.blend, hi, v))
        };
        inside(self.min.x, self.max.x, p.x)
            * inside(self.min.y, self.max.y, p.y)
            * inside(self.min.z, self.max.z, p.z)
    }

    /// The blend the shader takes: never 0, which would divide by it.
    pub fn blend_or_min(&self) -> f32 {
        self.blend.max(0.01)
    }
}

/// How much a point is in each of the game's light zones: the open air,
/// under the dome, inside the Hull. They add up to 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Whereabouts {
    pub outside: f32,
    pub dome: f32,
    pub hull: f32,
}

/// How much `p` is outside, under the dome and in the Hull, the later zone
/// in [`light_zones`] winning over the earlier, as in the shader.
pub fn places_at(zones: &[Zone], p: Vec3) -> Whereabouts {
    let mut dome = zones.first().map_or(0.0, |z| z.weight(p));
    let hull = zones.get(1).map_or(0.0, |z| z.weight(p));
    dome *= 1.0 - hull;
    Whereabouts {
        outside: 1.0 - dome - hull,
        dome,
        hull,
    }
}

/// Where the light isn't the open air's (docs/look-and-feel.md), in the
/// game's frame. Under the dome the sun comes through filtered and warm;
/// inside the Hull it's the sodium work lamps' warm fill.
pub fn light_zones() -> Vec<Zone> {
    vec![
        Zone {
            min: Vec3::new(-54.0, -5.0, -56.0),
            max: Vec3::new(34.0, 30.0, 44.0),
            blend: 3.0,
            ambient: [232, 200, 170],
            brightness: 0.32,
            sun: [255, 232, 205],
        },
        Zone {
            min: Vec3::new(-40.0, -5.0, -12.0),
            max: Vec3::new(8.0, 11.0, 20.0),
            blend: 1.5,
            ambient: [255, 186, 130],
            brightness: 0.8,
            sun: [255, 240, 220],
        },
    ]
}

/// The sky's colour at the horizon: thin dusty air lit by a red sun.
pub const SKY_HORIZON: Rgb = [246, 204, 150];
/// The light the sun gives: a red dwarf's, warm orange.
pub const SUNLIGHT: Rgb = [255, 182, 128];
/// How bright the sun is.
pub const SUN_BRIGHTNESS: f32 = 1.3;
/// The fill from the dusty sky: dimmer than the sun, and violet.
pub const AMBIENT: Rgb = [206, 186, 222];
pub const AMBIENT_BRIGHTNESS: f32 = 0.26;
/// How far off the sun and the planets are drawn.
pub const BODY_DISTANCE: f32 = 19300.0;
/// Half the width of the square round the view where things cast shadows,
/// in metres.
pub const SHADOW_RANGE: f32 = 45.0;

/// The dust in the air as the shaders draw it, starting as the look's
/// haze. Change it and the shaders follow: a dust storm closes it in and
/// turns it brown. `veil` is how much of the dust hides what's past the
/// haze's end (the sky, the sun and the planets): 0 in clear air.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Haze {
    pub color: Rgb,
    pub distance: f32,
    pub end: f32,
    pub veil: f32,
}

impl Default for Haze {
    fn default() -> Self {
        Look::default().haze()
    }
}

/// A fixed lamp as the shader lights by it: its colour at its intensity,
/// and its reach. The Go shader compiles these in as constants; here they
/// become the renderer's point lights, every one of them, with no camera
/// budget (see `plugin::lamp_point_light`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LampLight {
    /// The colour times the intensity, 0 to 1 per channel before scaling.
    pub color: Vec3,
    pub range: f32,
}

/// How a lamp from the world's layout lights: `render.PointLight`'s
/// intensity 0 means 1. A lamp with no reach gives nothing.
pub fn lamp_light(lamp: &crate::world::Lamp) -> Option<LampLight> {
    if lamp.range <= 0.0 {
        return None;
    }
    let k = if lamp.intensity == 0.0 {
        1.0
    } else {
        lamp.intensity
    };
    Some(LampLight {
        color: Vec3::from(lamp.color) * k,
        range: lamp.range,
    })
}

/// Bevy stores a point light's colour times its intensity over 4π
/// (luminous power to intensity). Scaling the game's intensity by this
/// hands the shader the game's `color * intensity` unchanged.
pub const LAMP_INTENSITY_SCALE: f32 = 4.0 * core::f32::consts::PI;

/// A lamp's light at a surface: the pool's share, 0 to 1 before its
/// colour, from its falloff `fall` (1 at the lamp, 0 at its reach) and how
/// much the surface faces it. The shader's `worldLamps`, for tests.
pub fn lamp_pool(fall: f32, facing: f32) -> f32 {
    let a = fall * fall * facing.max(0.0);
    0.45 * smoothstep(0.0, 0.12, a) + 0.55 * smoothstep(0.15, 0.5, a)
}

/// A headlamp. It shines along its transform's forward direction. Angles
/// are half angles in degrees: full brightness inside `inner`, fading to
/// `outer`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SpotLight {
    pub color: Rgb,
    pub intensity: f32,
    pub range: f32,
    pub inner: f32,
    pub outer: f32,
    pub enabled: bool,
    /// Higher priority keeps the driver's beams available in a crowd.
    pub priority: i32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: [0, 0, 0],
            intensity: 0.0,
            range: 0.0,
            inner: 0.0,
            outer: 0.0,
            enabled: false,
            priority: 0,
        }
    }
}

/// What the headlamps are gathered round: the camera. The viewer stamps
/// this on each `Camera3d` with its order; the highest order is the eye.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct BeamEye {
    pub order: isize,
}

/// A headlamp as the shader gets it this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub position: Vec3,
    pub direction: Vec3,
    pub range: f32,
    /// The colour times the intensity.
    pub color: Vec3,
    /// Cosines of the outer and inner half angles.
    pub cos_outer: f32,
    pub cos_inner: f32,
}

/// This frame's headlamps, at most [`MAX_SPOTS`] of them.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Beams(pub Vec<Beam>);

/// Picks the headlamps the eye sees by: those in reach (their range plus 80
/// m), the higher priority first, then the nearer.
pub fn pick_beams<'a>(
    eye: Vec3,
    lights: impl IntoIterator<Item = (&'a SpotLight, &'a GlobalTransform)>,
) -> Vec<Beam> {
    let mut nearby: Vec<(SpotLight, GlobalTransform, f32)> = lights
        .into_iter()
        .filter(|(l, _)| l.enabled && l.range > 0.0 && l.intensity > 0.0)
        .filter_map(|(l, tr)| {
            let d = eye.distance(tr.translation());
            (d < l.range + 80.0).then_some((*l, *tr, d))
        })
        .collect();
    nearby.sort_by(|a, b| b.0.priority.cmp(&a.0.priority).then(a.2.total_cmp(&b.2)));
    nearby.truncate(MAX_SPOTS);
    nearby
        .into_iter()
        .map(|(l, tr, _)| Beam {
            position: tr.translation(),
            direction: tr.forward().as_vec3(),
            range: l.range,
            color: scaled(l.color, l.intensity),
            cos_outer: l.outer.to_radians().cos(),
            cos_inner: l.inner.to_radians().cos(),
        })
        .collect()
}

/// Runs after transforms, so beams follow the drawn vehicle between
/// physics steps. It keeps the nearest lights in their own budget, leaving
/// the world's point lights to the lamps.
pub fn gather_spots(
    lights: Query<(&SpotLight, &GlobalTransform)>,
    eyes: Query<(&BeamEye, &GlobalTransform)>,
    mut beams: ResMut<Beams>,
) {
    let eye = eyes
        .iter()
        .max_by_key(|(eye, _)| eye.order)
        .map(|(_, tr)| tr.translation());
    let picked = match eye {
        Some(eye) => pick_beams(eye, lights.iter()),
        None => Vec::new(),
    };
    // Only a change marks the resource changed, so the materials aren't
    // rewritten every frame for nothing.
    if beams.0 != picked {
        beams.0 = picked;
    }
}

/// The sun's shadows: full (the platform's map, softly filtered), low (a
/// quarter of the texels, the cheap filter), or none.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowQuality {
    #[default]
    Full,
    Low,
    Off,
}

impl ShadowQuality {
    /// The shadow map's width in texels: 4096 on desktop, 1024 in the
    /// browser (the Go `shadowSize`), and half that on low. 0 for none.
    pub fn map_size(self) -> usize {
        let full = if cfg!(target_arch = "wasm32") {
            1024
        } else {
            4096
        };
        match self {
            Self::Full => full,
            Self::Low => full / 2,
            Self::Off => 0,
        }
    }
}

/// The GLSL `smoothstep`.
pub fn smoothstep(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `a` towards `b` by `t`, per channel.
pub fn mix_colour(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// The look, as a plugin. It goes after the default plugins; with the
/// `viewer` feature it registers the painted and outline materials and
/// the systems that feed them.
#[derive(Debug, Clone, Default)]
pub struct ShadingPlugin {
    pub look: Look,
}

impl ShadingPlugin {
    /// The game's look.
    pub fn earth_two() -> Self {
        Self {
            look: Look::earth_two(),
        }
    }
}

impl Plugin for ShadingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.look.haze())
            .insert_resource(self.look.clone())
            .init_resource::<Beams>()
            .init_resource::<ShadowQuality>()
            .add_systems(PostUpdate, gather_spots.after(TransformSystems::Propagate));
        #[cfg(feature = "viewer")]
        plugin::build(app);
    }
}
