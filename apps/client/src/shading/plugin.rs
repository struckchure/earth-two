//! The painted and outline materials, and the systems that feed them: the
//! rendering half of the look (see the module's `mod.rs`).
//!
//! The toon shader is a `MaterialExtension` on `StandardMaterial`, so the
//! renderer's own vertex path (skinning, morphs), its texture bindings,
//! its directional shadow maps and its clustered point lights all serve
//! it; only the lighting is the game's. The Go shader did its maths on
//! 8-bit sRGB values straight from the textures, so the fragment shader
//! encodes the renderer's linear colours to sRGB first and decodes its
//! result after, and the camera is left untonemapped: the look is the
//! shader's alone.

use std::collections::HashMap;

use bevy::{
    asset::embedded_asset,
    light::{
        CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
        NotShadowCaster, ShadowFilteringMethod,
    },
    material::OpaqueRendererMethod,
    mesh::{MeshVertexBufferLayoutRef, skinning::SkinnedMesh},
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialPipeline, MaterialPipelineKey, MaterialPlugin,
    },
    prelude::*,
    render::{
        render_resource::{
            AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
        },
        view::{DebandDither, Tonemapping},
    },
    shader::ShaderRef,
};

use super::{
    AMBIENT, AMBIENT_BRIGHTNESS, BeamEye, Beams, Haze, LAMP_INTENSITY_SCALE, Look, MAX_SPOTS,
    MAX_ZONES, OUTLINE_REACH, Rgb, SHADOW_RANGE, SUN_BRIGHTNESS, SUNLIGHT, ShadowQuality,
    lamp_light, rgb, scaled,
};
use crate::world::Lamp;

const PAINTED_SHADER: &str = "embedded://earth_two_client/shading/painted.wesl";
const OUTLINE_SHADER: &str = "embedded://earth_two_client/shading/outline.wesl";

/// The toon shader's settings, laid out in vec4s for WebGL2's uniform
/// alignment. What each lane carries is spelt out in `painted.wesl`.
#[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
pub struct PaintedUniform {
    /// rgb, w = softness.
    pub shadow_color: Vec4,
    /// rgb, w = rim power.
    pub rim_color: Vec4,
    /// rgb, w = fog distance.
    pub fog_color: Vec4,
    /// rgb at its brightness, w = 1 to light from sky and ground.
    pub ground_fill: Vec4,
    /// rgb at its brightness, w = exposure.
    pub ambient: Vec4,
    /// rgb at its brightness, w = knee.
    pub sun_color: Vec4,
    /// xyz = the way the sun shines, w = mid band.
    pub sun_dir: Vec4,
    /// x = rim threshold, y = fog end, z = veil, w = zone count.
    pub params: Vec4,
    /// x = material flags (1: smooth), y = spot count.
    pub flags: UVec4,
    /// xyz, w = blend.
    pub zone_min: [Vec4; MAX_ZONES],
    pub zone_max: [Vec4; MAX_ZONES],
    pub zone_ambient: [Vec4; MAX_ZONES],
    pub zone_sun: [Vec4; MAX_ZONES],
    /// xyz, w = range.
    pub spot_pos: [Vec4; MAX_SPOTS],
    pub spot_dir: [Vec4; MAX_SPOTS],
    pub spot_color: [Vec4; MAX_SPOTS],
    /// x = cos outer, y = cos inner.
    pub spot_cone: [Vec4; MAX_SPOTS],
}

impl Default for PaintedUniform {
    fn default() -> Self {
        Self {
            shadow_color: Vec4::ZERO,
            rim_color: Vec4::ZERO,
            fog_color: Vec4::ZERO,
            ground_fill: Vec4::ZERO,
            ambient: Vec4::new(0.0, 0.0, 0.0, 1.0),
            sun_color: Vec4::ZERO,
            sun_dir: Vec4::new(0.0, -1.0, 0.0, 0.0),
            params: Vec4::ZERO,
            flags: UVec4::ZERO,
            zone_min: [Vec4::ZERO; MAX_ZONES],
            zone_max: [Vec4::ZERO; MAX_ZONES],
            zone_ambient: [Vec4::ZERO; MAX_ZONES],
            zone_sun: [Vec4::ZERO; MAX_ZONES],
            spot_pos: [Vec4::ZERO; MAX_SPOTS],
            spot_dir: [Vec4::ZERO; MAX_SPOTS],
            spot_color: [Vec4::ZERO; MAX_SPOTS],
            spot_cone: [Vec4::ZERO; MAX_SPOTS],
        }
    }
}

/// The smooth flag: the ground is lit smoothly, with no bands, as its
/// slopes in bands read as stains (the Go `shading.Smooth` material).
pub const PAINTED_SMOOTH: u32 = 1;

/// The toon shader as a material extension. Its uniform is the same on
/// every painted material but for the flags; `sync_materials` keeps them
/// all current.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[reflect(Default)]
pub struct Painted {
    #[uniform(100)]
    #[reflect(ignore)]
    pub uniform: PaintedUniform,
}

impl MaterialExtension for Painted {
    fn fragment_shader() -> ShaderRef {
        PAINTED_SHADER.into()
    }
}

/// A surface painted with the toon shader, over the standard material's
/// textures and colours.
pub type PaintedMaterial = ExtendedMaterial<StandardMaterial, Painted>;

/// Marks an entity whose standard material is to be painted smoothly: the
/// ground.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Smooth;

/// Marks a model drawn with an outline: every mesh under it gets an
/// inverted hull (people, not the world: the world is painted, not inked).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Outlined;

/// The hull drawn round an outlined mesh: a child of the mesh's entity.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct OutlineHull;

/// The outline shader's settings, in vec4s.
#[derive(ShaderType, Debug, Clone, Copy, PartialEq, Default)]
pub struct OutlineUniform {
    /// The surface's own tint, as its standard material had it.
    pub base_color: Vec4,
    /// rgb, w = the outline's width in pixels.
    pub outline_color: Vec4,
    /// rgb, w = fog distance.
    pub fog_color: Vec4,
    /// x = fog end, y = reach.
    pub params: Vec4,
}

/// The outline pass's material: the model's back faces pushed out along
/// their normals, in a dark tint of the surface's own colour.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[reflect(Default)]
pub struct OutlineMaterial {
    #[uniform(0)]
    #[reflect(ignore)]
    pub uniform: OutlineUniform,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl Material for OutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        OUTLINE_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        OUTLINE_SHADER.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The hull is the back faces, so the front ones are culled.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// The sun and the sky's fill as the shaders light by them: the Go
/// `render.DirectionalLight` and `render.AmbientLight` resources. The
/// direction comes from the scene's `DirectionalLight` entity, which also
/// casts the shadows. Starts as the game's.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Daylight {
    pub sun_color: Rgb,
    /// Scales the sun's colour; 0 means 1.
    pub sun_brightness: f32,
    pub ambient_color: Rgb,
    pub ambient_brightness: f32,
}

impl Default for Daylight {
    fn default() -> Self {
        Self {
            sun_color: SUNLIGHT,
            sun_brightness: SUN_BRIGHTNESS,
            ambient_color: AMBIENT,
            ambient_brightness: AMBIENT_BRIGHTNESS,
        }
    }
}

/// The painted uniform as it was last sent, so materials are only
/// rewritten when something changed.
#[derive(Resource, Debug, Default)]
struct Sent {
    painted: Option<PaintedUniform>,
    haze: Option<Haze>,
}

/// Painted materials by the standard material they came from, so meshes
/// that shared one still do.
#[derive(Resource, Debug, Default)]
struct PaintedCache(HashMap<(AssetId<StandardMaterial>, bool), Handle<PaintedMaterial>>);

pub(super) fn build(app: &mut App) {
    embedded_asset!(app, "painted.wesl");
    embedded_asset!(app, "outline.wesl");
    app.add_plugins((
        MaterialPlugin::<PaintedMaterial>::default(),
        MaterialPlugin::<OutlineMaterial>::default(),
    ))
    .init_resource::<Daylight>()
    .init_resource::<Sent>()
    .init_resource::<PaintedCache>()
    .add_systems(
        Update,
        (
            mark_cameras,
            apply_shadow_quality,
            attach_lamps,
            adopt_materials,
        ),
    )
    .add_systems(PostUpdate, sync_materials.after(super::gather_spots));
}

/// The point light a lamp from the layout becomes: the game's own
/// intensity scale and range, never a camera budget. The colour is handed
/// over as linear so the shader sees the manifest's numbers unchanged.
pub fn lamp_point_light(lamp: &Lamp) -> Option<PointLight> {
    let light = lamp_light(lamp)?;
    let peak = light.color.max_element().max(f32::EPSILON);
    Some(PointLight {
        color: Color::linear_rgb(
            light.color.x / peak,
            light.color.y / peak,
            light.color.z / peak,
        ),
        intensity: peak * LAMP_INTENSITY_SCALE,
        range: light.range,
        radius: 0.0,
        shadow_maps_enabled: false,
        ..default()
    })
}

/// The cascade config for the sun's shadows: one map, over the square
/// round the view the Go renderer shadowed (`shadowRange` either side).
pub fn sun_cascades() -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        num_cascades: 1,
        minimum_distance: 0.1,
        maximum_distance: 2.0 * SHADOW_RANGE,
        first_cascade_far_bound: 2.0 * SHADOW_RANGE,
        overlap_proportion: 0.0,
    }
    .build()
}

/// Gives each lamp from the layout its light.
fn attach_lamps(mut commands: Commands, lamps: Query<(Entity, &Lamp), Added<Lamp>>) {
    for (entity, lamp) in &lamps {
        if let Some(light) = lamp_point_light(lamp) {
            commands.entity(entity).insert(light);
        }
    }
}

/// Cameras see the look as the shader paints it: no tonemapping, no
/// dither; they're the headlamps' eye; and their shadow filter follows the
/// quality.
fn mark_cameras(
    mut commands: Commands,
    cameras: Query<(Entity, &Camera), Added<Camera3d>>,
    quality: Res<ShadowQuality>,
) {
    for (entity, camera) in &cameras {
        commands.entity(entity).insert((
            Tonemapping::None,
            DebandDither::Disabled,
            BeamEye {
                order: camera.order,
            },
            filter_for(*quality),
        ));
    }
}

fn filter_for(quality: ShadowQuality) -> ShadowFilteringMethod {
    match quality {
        ShadowQuality::Full => ShadowFilteringMethod::Gaussian,
        ShadowQuality::Low | ShadowQuality::Off => ShadowFilteringMethod::Hardware2x2,
    }
}

/// Applies the shadow quality: the map's size, whether the sun casts
/// shadows at all, and the cameras' filter.
fn apply_shadow_quality(
    quality: Res<ShadowQuality>,
    mut map: ResMut<DirectionalLightShadowMap>,
    mut suns: Query<&mut DirectionalLight>,
    mut cameras: Query<&mut ShadowFilteringMethod, With<Camera3d>>,
) {
    if !quality.is_changed() {
        return;
    }
    let size = quality.map_size();
    if size > 0 {
        map.size = size;
    }
    for mut sun in &mut suns {
        sun.shadow_maps_enabled = size > 0;
    }
    for mut filter in &mut cameras {
        *filter = filter_for(*quality);
    }
}

/// Paints every standard material as it appears: the mesh gets the painted
/// material in its place, and, under an outlined model, a hull child.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn adopt_materials(
    mut commands: Commands,
    meshes: Query<(
        Entity,
        &MeshMaterial3d<StandardMaterial>,
        Option<&Mesh3d>,
        Option<&SkinnedMesh>,
        Has<Smooth>,
    )>,
    parents: Query<&ChildOf>,
    outlined: Query<(), With<Outlined>>,
    standard: Res<Assets<StandardMaterial>>,
    mut painted: ResMut<Assets<PaintedMaterial>>,
    mut outlines: ResMut<Assets<OutlineMaterial>>,
    mut cache: ResMut<PaintedCache>,
    look: Res<Look>,
    haze: Res<Haze>,
    sent: Res<Sent>,
) {
    for (entity, material, mesh, skin, smooth) in &meshes {
        // Not loaded yet: next frame.
        let Some(base) = standard.get(&material.0) else {
            continue;
        };
        let handle = cache
            .0
            .entry((material.0.id(), smooth))
            .or_insert_with(|| {
                let mut uniform = sent.painted.unwrap_or_default();
                uniform.flags.x = if smooth { PAINTED_SMOOTH } else { 0 };
                painted.add(PaintedMaterial {
                    base: StandardMaterial {
                        // The shader cuts out what's less than half opaque
                        // in the shadow map, and dithers the rest itself.
                        alpha_mode: AlphaMode::Mask(0.5),
                        opaque_render_method: OpaqueRendererMethod::Forward,
                        ..base.clone()
                    },
                    extension: Painted { uniform },
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));

        let inked = outlined.contains(entity)
            || parents
                .iter_ancestors(entity)
                .any(|ancestor| outlined.contains(ancestor));
        let Some(mesh) = mesh.filter(|_| inked) else {
            continue;
        };
        let hull = outlines.add(OutlineMaterial {
            uniform: outline_uniform(&look, &haze, base.base_color.to_linear().to_vec4()),
            texture: base.base_color_texture.clone(),
        });
        let mut child = commands.spawn((
            OutlineHull,
            Name::new("outline"),
            mesh.clone(),
            MeshMaterial3d(hull),
            NotShadowCaster,
            ChildOf(entity),
        ));
        if let Some(skin) = skin {
            child.insert(skin.clone());
        }
    }
}

fn outline_uniform(look: &Look, haze: &Haze, base_color: Vec4) -> OutlineUniform {
    OutlineUniform {
        base_color,
        outline_color: rgb(look.outline_color).extend(look.outline_width),
        fog_color: rgb(haze.color).extend(haze.distance),
        params: Vec4::new(haze.end, OUTLINE_REACH, 0.0, 0.0),
    }
}

/// The uniform for this frame: the look, the daylight, the haze as it is
/// now, the sun's direction and the headlamps.
fn painted_uniform(
    look: &Look,
    daylight: &Daylight,
    haze: &Haze,
    sun_dir: Vec3,
    beams: &Beams,
) -> PaintedUniform {
    let mut u = PaintedUniform {
        shadow_color: rgb(look.shadow_color).extend(look.softness),
        rim_color: rgb(look.rim_color).extend(look.rim_power),
        fog_color: rgb(haze.color).extend(haze.distance),
        ground_fill: scaled(look.ground_fill, look.ground_brightness).extend(
            if look.ground_fill == [0, 0, 0] {
                0.0
            } else {
                1.0
            },
        ),
        ambient: scaled(daylight.ambient_color, daylight.ambient_brightness)
            .extend(look.exposure_or_one()),
        sun_color: scaled(
            daylight.sun_color,
            if daylight.sun_brightness == 0.0 {
                1.0
            } else {
                daylight.sun_brightness
            },
        )
        .extend(look.knee),
        sun_dir: sun_dir.extend(look.mid_band),
        params: Vec4::new(
            look.rim_threshold,
            haze.end,
            haze.veil,
            look.zones.len().min(MAX_ZONES) as f32,
        ),
        flags: UVec4::new(0, beams.0.len() as u32, 0, 0),
        ..default()
    };
    for (i, z) in look.zones.iter().take(MAX_ZONES).enumerate() {
        u.zone_min[i] = z.min.extend(z.blend_or_min());
        u.zone_max[i] = z.max.extend(0.0);
        u.zone_ambient[i] = scaled(z.ambient, z.brightness).extend(0.0);
        u.zone_sun[i] = rgb(z.sun).extend(0.0);
    }
    for (i, b) in beams.0.iter().take(MAX_SPOTS).enumerate() {
        u.spot_pos[i] = b.position.extend(b.range);
        u.spot_dir[i] = b.direction.extend(0.0);
        u.spot_color[i] = b.color.extend(0.0);
        u.spot_cone[i] = Vec4::new(b.cos_outer, b.cos_inner, 0.0, 0.0);
    }
    u
}

/// Sends the frame's settings to every painted material, and the haze to
/// every outline, when they've changed.
#[allow(clippy::too_many_arguments)]
fn sync_materials(
    look: Res<Look>,
    daylight: Res<Daylight>,
    haze: Res<Haze>,
    beams: Res<Beams>,
    suns: Query<&GlobalTransform, With<DirectionalLight>>,
    mut sent: ResMut<Sent>,
    mut painted: ResMut<Assets<PaintedMaterial>>,
    mut outlines: ResMut<Assets<OutlineMaterial>>,
) {
    // Straight down until there's a sun.
    let sun_dir = suns
        .iter()
        .next()
        .map_or(Vec3::NEG_Y, |tr| tr.forward().as_vec3());
    let uniform = painted_uniform(&look, &daylight, &haze, sun_dir, &beams);
    if sent.painted != Some(uniform) {
        sent.painted = Some(uniform);
        for (_, material) in painted.iter_mut() {
            let flags = material.extension.uniform.flags.x;
            material.extension.uniform = uniform;
            material.extension.uniform.flags.x = flags;
        }
    }
    if sent.haze != Some(*haze) {
        sent.haze = Some(*haze);
        for (_, material) in outlines.iter_mut() {
            let base = material.uniform.base_color;
            material.uniform = outline_uniform(&look, &haze, base);
        }
    }
}
