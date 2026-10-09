//! The sky as it's drawn: the dome with the sky painted on it, the sun and
//! the neighbour planets out beyond the horizon, the stars as lines a
//! pixel long, the dust and sparks as soft discs in one mesh, the dust a
//! storm blows across the view, and the hour's light on Bevy's sun, fill
//! and haze. Everything here follows the resources the headless systems
//! keep (daylight.rs, weather.rs, effects.rs).

use super::{
    SkySystems,
    clock::Clock,
    colour::Rgba,
    daylight::{self, Daylight, SkyPaint, Sun},
    effects::{Effects, MAX_PARTICLES},
    skytex::{SKY_TEX_H, SKY_TEX_W, paint_sky, paint_sphere, pixel_dir, sun_surface, surface},
    stars::{STAR_BUDGET, STAR_PX, STARS_CROSSED, STARS_RADIUS, visible_stars},
    sun::{
        BODY_DISTANCE, NEIGHBOURS, SKY_RADIUS, SUN_DISTANCE, ecliptic, neighbour_at, sky_days,
        sun_radius,
    },
    time::UnixTime,
    weather::{Ambient, Haze, STORM_DUST, STORM_FROM, SunLight, Weather, dust_wind, outdoors},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
    window::PrimaryWindow,
};
use earth_two_world::terrain::{lattice, smoothstep};
use std::f64::consts::PI;

/// The Go shader's light scale, 1.3 for the sun past white, in lux: a
/// stand-in until the shading port maps the toon light itself.
pub const SUN_LUX_PER_UNIT: f32 = 8000.0;
/// The Go fill's scale in Bevy's ambient cd/m²: the same stand-in.
pub const AMBIENT_PER_UNIT: f32 = 300.0;
/// How far the camera must see to take in the dome.
pub const SKY_FAR: f32 = SKY_RADIUS * 1.04;

/// A part of the sky, and where it is from the camera: `dir` a unit
/// direction, at a distance, scaled to look `size` across.
#[derive(Component, Debug, Default)]
pub struct SkyBody {
    pub dir: Vec3,
    pub distance: f32,
    pub size: f32,
    /// The dome: centred on the camera, not off in a direction.
    pub dome: bool,
    /// Turns its top pole to the camera (the sun, whose face is painted
    /// from the middle of its disc out).
    pub facing: bool,
    /// Turns it about the ecliptic's pole, by this angle (a planet).
    pub spin: f32,
}

/// Marks a neighbour in the sky, by its index in NEIGHBOURS.
#[derive(Component, Debug)]
pub struct PlanetSky(pub usize);

/// The stars' mesh.
#[derive(Component, Debug)]
pub struct StarsMesh;

/// The particles' mesh.
#[derive(Component, Debug)]
pub struct EffectsMesh;

/// The blown dust over the view: its cast, its gusts (by gust and step)
/// and its motes.
#[derive(Component, Debug)]
pub struct DustCast;
#[derive(Component, Debug)]
pub struct DustGust(usize, usize);
#[derive(Component, Debug)]
pub struct DustMote(usize);

/// The sky being painted off the main thread, if it is: what for, and the
/// dome's image to put it in.
#[derive(Resource, Default)]
pub struct SkyPainting {
    task: Option<(SkyPaint, Task<Vec<u8>>)>,
    dome: Handle<Image>,
}

pub struct SkyRenderPlugin;

impl Plugin for SkyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyPainting>()
            .add_systems(
                Startup,
                (
                    spawn_sky.after(daylight::spawn_sun),
                    spawn_dust,
                    set_clear_colour,
                ),
            )
            .add_systems(
                Update,
                (far_enough, sync_light, repaint_sky, draw_dust).after(SkySystems),
            )
            .add_systems(
                PostUpdate,
                (move_sky, draw_stars, draw_effects).before(TransformSystems::Propagate),
            );
    }
}

/// The Go client's clear colour, under everything.
fn set_clear_colour(mut clear: ResMut<ClearColor>) {
    clear.0 = Rgba {
        r: 250,
        g: 196,
        b: 120,
        a: 255,
    }
    .to_bevy();
}

/// A unit sphere with its poles on Y and a plain longitude-latitude wrap,
/// the one skytex.rs paints for.
pub fn sphere_mesh(sectors: usize, stacks: usize) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for i in 0..=stacks {
        let v = i as f64 / stacks as f64;
        let el = PI * (0.5 - v);
        for j in 0..=sectors {
            let u = j as f64 / sectors as f64;
            let az = 2.0 * PI * u;
            positions.push([
                (el.cos() * az.cos()) as f32,
                el.sin() as f32,
                (el.cos() * az.sin()) as f32,
            ]);
            uvs.push([u as f32, v as f32]);
        }
    }
    let normals = positions.clone();
    let mut indices = Vec::new();
    let row = (sectors + 1) as u32;
    for i in 0..stacks as u32 {
        for j in 0..sectors as u32 {
            let (a, b) = (i * row + j, (i + 1) * row + j);
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// A texture of `w` by `h` sRGB pixels, filtered bilinearly.
pub fn upload_texture(pixels: Vec<u8>, w: usize, h: usize) -> Image {
    Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Adds the sky: the dome, the sun, and the planets, and the stars' and
/// the particles' meshes. None of it casts shadows: it's all far beyond
/// what it would shade.
fn spawn_sky(
    mut commands: Commands,
    day: Res<Daylight>,
    mut painting: ResMut<SkyPainting>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let sphere = meshes.add(sphere_mesh(96, 48));
    let unlit = |materials: &mut Assets<StandardMaterial>, tex: Handle<Image>| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(tex),
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })
    };
    // The dome, seen from within, with the gradient down it.
    let paint = day.paint();
    let dome = images.add(upload_texture(
        paint_sky(paint.dir, paint.dusk, paint.night, paint.turn),
        SKY_TEX_W,
        SKY_TEX_H,
    ));
    painting.dome = dome.clone();
    commands.spawn((
        Name::new("sky dome"),
        Mesh3d(sphere.clone()),
        MeshMaterial3d(unlit(&mut materials, dome)),
        Transform::IDENTITY,
        SkyBody {
            dome: true,
            size: SKY_RADIUS,
            ..default()
        },
        NotShadowCaster,
        NotShadowReceiver,
        NoFrustumCulling,
    ));
    // The sun: a sphere with its face painted on (skytex.rs), its top pole
    // turned to the camera. Its glare is painted into the dome behind it.
    let r = sun_radius() as f32;
    let face = images.add(upload_texture(
        paint_sphere(SKY_TEX_W, SKY_TEX_H, sun_surface),
        SKY_TEX_W,
        SKY_TEX_H,
    ));
    commands.spawn((
        Name::new("sun disc"),
        Mesh3d(sphere.clone()),
        MeshMaterial3d(unlit(&mut materials, face)),
        Transform::IDENTITY,
        SkyBody {
            dir: day.sun,
            distance: SUN_DISTANCE,
            size: SUN_DISTANCE * f64::from(r).tan() as f32,
            facing: true,
            ..default()
        },
        NotShadowCaster,
        NotShadowReceiver,
        NoFrustumCulling,
    ));
    // The neighbours, their surfaces painted on (skytex.rs) and lit by the
    // scene's light, so they show phases.
    for (i, n) in NEIGHBOURS.iter().enumerate() {
        let Some(s) = surface(n.name) else { continue };
        let tex = images.add(upload_texture(
            paint_sphere(SKY_TEX_W, SKY_TEX_H, s),
            SKY_TEX_W,
            SKY_TEX_H,
        ));
        commands.spawn((
            Name::new(format!("planet {}", n.name)),
            Mesh3d(sphere.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(tex),
                fog_enabled: false,
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::IDENTITY,
            SkyBody {
                distance: BODY_DISTANCE,
                ..default()
            },
            PlanetSky(i),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
    // The stars: lines, a pixel long, in their own colours, blended over
    // the dome and behind the world.
    let lines = Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    commands.spawn((
        Name::new("stars"),
        StarsMesh,
        Mesh3d(meshes.add(lines)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::IDENTITY,
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        NoFrustumCulling,
    ));
    // The particles: quads facing the camera, each a soft disc in its
    // vertex colour, its edge fading out.
    let disc = images.add(upload_texture(soft_disc(64), 64, 64));
    let mut quads = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let mut uvs = Vec::with_capacity(4 * MAX_PARTICLES);
    let mut indices = Vec::with_capacity(6 * MAX_PARTICLES);
    for i in 0..MAX_PARTICLES as u32 {
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        let b = 4 * i;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    quads.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0f32; 3]; 4 * MAX_PARTICLES],
    );
    quads.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 4 * MAX_PARTICLES]);
    quads.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    quads.insert_indices(Indices::U32(indices));
    commands.spawn((
        Name::new("effects"),
        EffectsMesh,
        Mesh3d(meshes.add(quads)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(disc),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })),
        Transform::IDENTITY,
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        NoFrustumCulling,
    ));
}

/// The particles' disc: white, soft all the way in (dust thins out from
/// its middle, it has no rim), what the Go fragment shader drew.
fn soft_disc(n: usize) -> Vec<u8> {
    let mut pixels = vec![255u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (u, v) = ((x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32);
            let r = Vec2::new(u * 2.0 - 1.0, v * 2.0 - 1.0).length();
            let a = f64::from(1.0 - smoothstep(0.0, 1.0, r)).powf(1.6) as f32;
            pixels[(y * n + x) * 4 + 3] = (255.0 * a) as u8;
        }
    }
    pixels
}

/// The camera the sky's drawn round, if there is one.
fn eye(cameras: &Query<&Transform, With<Camera3d>>) -> Option<Transform> {
    cameras.iter().next().copied()
}

/// Keeps the camera's far plane past the dome: the sky's drawn as if
/// infinitely far away, out beyond the world's edge.
fn far_enough(mut projections: Query<&mut Projection, With<Camera3d>>) {
    for mut projection in &mut projections {
        if let Projection::Perspective(p) = &mut *projection
            && p.far < SKY_FAR
        {
            p.far = SKY_FAR;
        }
    }
}

/// Puts the hour's and the weather's light on Bevy's sun, fill and haze.
fn sync_light(
    mut commands: Commands,
    sun: Res<SunLight>,
    ambient: Res<Ambient>,
    haze: Res<Haze>,
    mut suns: Query<(Entity, Option<&mut DirectionalLight>), With<Sun>>,
    mut global: ResMut<GlobalAmbientLight>,
    mut cameras: Query<(Entity, Option<&mut DistanceFog>), With<Camera3d>>,
) {
    for (entity, light) in &mut suns {
        match light {
            Some(mut light) => {
                light.color = sun.color.to_bevy();
                light.illuminance = sun.brightness * SUN_LUX_PER_UNIT;
            }
            None => {
                commands.entity(entity).insert(DirectionalLight {
                    color: sun.color.to_bevy(),
                    illuminance: sun.brightness * SUN_LUX_PER_UNIT,
                    shadow_maps_enabled: true,
                    ..default()
                });
            }
        }
    }
    global.color = ambient.color.to_bevy();
    global.brightness = ambient.brightness * AMBIENT_PER_UNIT;
    // The Go haze: a share 1 - e^(-d / distance) of the way to its colour.
    let fog = DistanceFog {
        color: haze.color.to_bevy(),
        directional_light_color: Color::NONE,
        directional_light_exponent: 8.0,
        falloff: FogFalloff::Exponential {
            density: 1.0 / haze.distance.max(1.0),
        },
    };
    for (entity, current) in &mut cameras {
        match current {
            Some(mut current) => *current = fog.clone(),
            None => {
                commands.entity(entity).insert(fog.clone());
            }
        }
    }
}

/// Paints the sky again, off the main thread, once the sun's moved on, or
/// the stars' sky has turned, or dusk or night's come on, enough to see;
/// and puts a finished painting up.
fn repaint_sky(
    mut day: ResMut<Daylight>,
    mut painting: ResMut<SkyPainting>,
    mut images: ResMut<Assets<Image>>,
) {
    if let Some((paint, mut task)) = painting.task.take() {
        let Some(pixels) = block_on(poll_once(&mut task)) else {
            painting.task = Some((paint, task));
            return; // still painting
        };
        if let Some(mut image) = images.get_mut(&painting.dome) {
            image.data = Some(pixels);
        }
        day.baked = paint;
    }
    if !day.wants_repaint() {
        return;
    }
    let paint = day.paint();
    let task = AsyncComputeTaskPool::get()
        .spawn(async move { paint_sky(paint.dir, paint.dusk, paint.night, paint.turn) });
    painting.task = Some((paint, task));
}

/// Keeps the sky round the camera, and the planets where they are now.
fn move_sky(
    day: Res<Daylight>,
    cameras: Query<&Transform, With<Camera3d>>,
    mut bodies: Query<
        (
            &mut Transform,
            &mut SkyBody,
            &mut Visibility,
            Option<&PlanetSky>,
        ),
        Without<Camera3d>,
    >,
) {
    let Some(eye) = eye(&cameras) else { return };
    let days = sky_days(UnixTime::now());
    let (s, v) = ecliptic(day.sun);
    let pole = s.cross(v).normalize_or_zero();
    for (mut tr, mut b, mut visible, planet) in &mut bodies {
        if let Some(PlanetSky(i)) = planet {
            let n = &NEIGHBOURS[*i];
            let (dir, radius) = neighbour_at(n, day.sun, days);
            b.dir = dir;
            b.size = b.distance * radius.tan() as f32;
            // Each turns once an orbit, held face-on to its star like the Red.
            b.spin = (2.0 * PI * (days / n.p).rem_euclid(1.0)) as f32;
        }
        if b.dome {
            tr.translation = eye.translation;
            tr.scale = Vec3::splat(b.size);
            continue;
        }
        if b.facing {
            b.dir = day.sun; // the sun, where it's got to
            // Set, well under the horizon.
            *visible = if day.sun.y < -0.06 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
        tr.translation = eye.translation + b.dir * b.distance;
        tr.scale = Vec3::splat(b.size);
        tr.rotation = if b.facing {
            Quat::from_rotation_arc(Vec3::Y, -b.dir)
        } else {
            // Upright on the ecliptic's pole, turned by its spin.
            Quat::from_axis_angle(pole, b.spin) * Quat::from_rotation_arc(Vec3::Y, pole)
        };
    }
}

/// Draws the stars as many as it's night, and as the air's clear: their
/// mesh rebuilt round the camera.
#[allow(clippy::type_complexity)]
fn draw_stars(
    day: Res<Daylight>,
    haze: Res<Haze>,
    w: Res<Weather>,
    cameras: Query<&Transform, With<Camera3d>>,
    mut stars: Query<
        (&Mesh3d, &mut Transform, &mut Visibility),
        (With<StarsMesh>, Without<Camera3d>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(eye) = eye(&cameras) else { return };
    // Any dust in the air hides them: the veil in a storm, and the
    // thickening in a dusty spell.
    let clear_air = (1.0 - haze.veil) * (1.0 - smoothstep(0.0, STORM_FROM, w.storm));
    let dots = visible_stars(day.night, clear_air, day.turn, w.t, STAR_BUDGET);
    for (mesh, mut tr, mut visible) in &mut stars {
        if dots.is_empty() {
            *visible = Visibility::Hidden;
            continue;
        }
        *visible = Visibility::Inherited;
        tr.translation = eye.translation;
        let (right, up) = (eye.right().as_vec3(), eye.up().as_vec3());
        let mut positions = Vec::with_capacity(dots.len() * 8);
        let mut colours = Vec::with_capacity(dots.len() * 8);
        let mut line = |a: Vec3, b: Vec3, c: Rgba| {
            positions.push(a.to_array());
            positions.push(b.to_array());
            let c = c.to_linear().to_f32_array();
            colours.push(c);
            colours.push(c);
        };
        for s in &dots {
            let at = s.dir * STARS_RADIUS;
            line(at - right * s.size, at + right * s.size, s.colour);
            if !STARS_CROSSED {
                continue;
            }
            line(at - up * s.size, at + up * s.size, s.colour);
            if s.bright > 0.75 {
                // The brightest: a faint sparkle.
                let spike = STAR_PX * 5.0 * s.bright;
                let faint = s.colour.with_alpha((f32::from(s.colour.a) * 0.22) as u8);
                line(at - right * spike, at + right * spike, faint);
                line(at - up * spike, at + up * spike, faint);
            }
        }
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        }
    }
}

/// Draws the particles, far to near, facing the camera.
fn draw_effects(
    mut fx: ResMut<Effects>,
    cameras: Query<&Transform, With<Camera3d>>,
    mut quads: Query<(&Mesh3d, &mut Visibility), With<EffectsMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(eye) = eye(&cameras) else { return };
    if fx.ps.is_empty() && fx.drawn == 0 {
        return;
    }
    let ahead = eye.forward().as_vec3();
    let right = ahead.cross(eye.up().as_vec3()).normalize_or_zero();
    let up = right.cross(ahead);
    let mut verts = vec![[0.0f32; 3]; 4 * MAX_PARTICLES];
    let mut cols = vec![Rgba::default(); 4 * MAX_PARTICLES];
    let n = fx.quads(eye.translation, ahead, right, up, &mut verts, &mut cols);
    for (mesh, mut visible) in &mut quads {
        *visible = if n == 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts.clone());
            m.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                cols.iter()
                    .map(|c| c.to_linear().to_f32_array())
                    .collect::<Vec<_>>(),
            );
        }
    }
}

// Blown dust: DUST_MOTES motes blowing across the view on the wind, grit
// and the odd longer streak, nearer ones faster, longer and brighter;
// gusts, broad bands of thicker dust sweeping through; and over it all a
// brown cast. The wind blows from the east (from +X), so it crosses the
// view as the camera faces across it.
pub const DUST_MOTES: usize = 420;
pub const DUST_GUSTS: usize = 6;
pub const GUST_STEPS: usize = 8;

/// The blown dust's nodes, over the world and under the HUD, hidden until
/// a storm blows.
fn spawn_dust(mut commands: Commands) {
    let node = || Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    commands
        .spawn((
            Name::new("blown dust"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                overflow: Overflow::clip(),
                ..default()
            },
            GlobalZIndex(-10),
            Visibility::Hidden,
        ))
        .with_children(|dust| {
            dust.spawn((
                DustCast,
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..node()
                },
                BackgroundColor(Color::NONE),
            ));
            for i in 0..DUST_GUSTS {
                for j in 0..GUST_STEPS {
                    dust.spawn((DustGust(i, j), node(), BackgroundColor(Color::NONE)));
                }
            }
            for i in 0..DUST_MOTES {
                dust.spawn((
                    DustMote(i),
                    node(),
                    BackgroundColor(Color::NONE),
                    UiTransform::IDENTITY,
                    Visibility::Hidden,
                ));
            }
        });
}

fn sign(v: f32) -> f32 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// Brings `v` into [0, n).
fn wrapf(v: f32, n: f32) -> f32 {
    let v = v % n;
    if v < 0.0 { v + n } else { v }
}

/// Draws the blown dust, as hard as the storm and as much as the camera's
/// out in it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw_dust(
    w: Res<Weather>,
    day: Res<Daylight>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<&Transform, With<Camera3d>>,
    mut root: Query<&mut Visibility, (With<GlobalZIndex>, With<Children>, Without<DustMote>)>,
    mut cast: Query<&mut BackgroundColor, (With<DustCast>, Without<DustGust>, Without<DustMote>)>,
    mut gusts: Query<
        (&DustGust, &mut Node, &mut BackgroundColor),
        (Without<DustCast>, Without<DustMote>),
    >,
    mut motes: Query<
        (
            &DustMote,
            &mut Node,
            &mut BackgroundColor,
            &mut UiTransform,
            &mut Visibility,
        ),
        Without<DustCast>,
    >,
) {
    // By night the dust's as dark as the night.
    let dark = 1.0 - 0.8 * day.night;
    let dim = |c: Rgba| Rgba {
        r: (f32::from(c.r) * dark) as u8,
        g: (f32::from(c.g) * dark) as u8,
        b: (f32::from(c.b) * dark) as u8,
        a: c.a,
    };
    let eye = eye(&cameras);
    let window = windows.iter().next();
    let k = match (eye, window) {
        (Some(eye), Some(_)) if w.storm >= 0.02 => w.storm * outdoors(eye.translation),
        _ => 0.0,
    };
    let Some(root_visible) = root.iter_mut().find(|v| **v != Visibility::Visible) else {
        return;
    };
    let mut root_visible = root_visible;
    if k < 0.02 {
        *root_visible = Visibility::Hidden;
        return;
    }
    *root_visible = Visibility::Inherited;
    let (eye, window) = (eye.unwrap(), window.unwrap());
    let (width, height) = (window.width(), window.height());
    let right = eye.right().as_vec3();
    let ahead = eye.forward().as_vec3();
    let wind = dust_wind().normalize();
    // How the wind crosses the view: sideways across it, and how much it
    // blows into the camera's face (which reads as dust rushing past).
    let across = wind.dot(right);
    let into = -wind.dot(ahead);
    let t = w.t;
    // The cast.
    for mut c in &mut cast {
        c.0 = dim(STORM_DUST.with_alpha((70.0 * k) as u8)).to_bevy();
    }
    // The gusts: broad soft bands crossing the view, thickest in their
    // middle, in steps.
    for (DustGust(i, j), mut node, mut colour) in &mut gusts {
        let (i, j) = (*i, *j);
        let u = lattice(i as i32, 7, 0xd05);
        let speed = (0.25 + 0.35 * u) * width * (across + 0.25 * sign(across));
        let bw = width * (0.35 + 0.5 * lattice(i as i32, 11, 0xd05));
        let x = wrapf(u * (width + bw) + t * speed, width + bw) - bw;
        let y = height * (0.15 + 0.7 * lattice(i as i32, 13, 0xd05));
        let bh = height * (0.25 + 0.35 * lattice(i as i32, 17, 0xd05));
        let a = (28.0 * k * (0.5 + 0.5 * f64::from(t * 0.7 + i as f32).sin() as f32)) as u8;
        let f = (j as f32 + 0.5) / GUST_STEPS as f32;
        colour.0 = dim(STORM_DUST.with_alpha((f32::from(a) * (1.0 - (2.0 * f - 1.0).abs())) as u8))
            .to_bevy();
        node.left = px((x + bw * j as f32 / GUST_STEPS as f32) as i32);
        node.top = px((y - bh / 2.0) as i32);
        node.width = px((bw / GUST_STEPS as f32) as i32 + 1);
        node.height = px(bh as i32);
    }
    // The motes: most short grit, some longer streaks, each slanting its
    // own way and wobbling on the turbulence, in the dust's own colours.
    let n = (DUST_MOTES as f32 * k) as usize;
    for (DustMote(i), mut node, mut colour, mut ui, mut visible) in &mut motes {
        let i = *i;
        if i >= n {
            *visible = Visibility::Hidden;
            continue;
        }
        *visible = Visibility::Inherited;
        let depth = 0.2 + 0.8 * lattice(i as i32, 1, 0xd17);
        let (u, v) = (lattice(i as i32, 2, 0xd17), lattice(i as i32, 3, 0xd17));
        let long = lattice(i as i32, 4, 0xd17);
        let slant = (lattice(i as i32, 5, 0xd17) - 0.35) * 0.5;
        let vx = width * (0.5 + 1.1 * depth) * (across + 0.15 * sign(across));
        let vy = vx * slant * 0.3 + height * 0.03 * (1.0 + depth);
        let wob = f64::from(t * (2.0 + 3.0 * long) + i as f32 * 1.7).sin() as f32 * 18.0 * depth;
        let mut x = u * width + t * vx;
        let mut y = v * height + t * vy + wob;
        if into > 0.0 {
            // Blowing into the camera's face: out from the middle, growing.
            let (cx, cy) = (x - width / 2.0, y - height / 2.0);
            let grow = 1.0 + into * (f64::from(t * (0.6 + depth) + u * 7.0) % 1.5) as f32;
            x = width / 2.0 + cx * grow;
            y = height / 2.0 + cy * grow;
        }
        let (x, y) = (wrapf(x, width), wrapf(y, height));
        let mut streak = (2.0 + 6.0 * depth) * (across.abs() + 0.3);
        if long > 0.8 {
            streak *= 4.0 + 4.0 * depth;
        }
        let dx = streak * sign(vx) * (across.abs() + 0.15);
        let dy = dx * slant;
        let mut shade = super::colour::mix_colour(
            Rgba {
                r: 170,
                g: 112,
                b: 76,
                a: 255,
            },
            Rgba {
                r: 222,
                g: 176,
                b: 134,
                a: 255,
            },
            lattice(i as i32, 6, 0xd17),
        );
        shade.a = ((25.0 + 75.0 * depth) * k) as u8;
        shade = dim(shade);
        // A thin rectangle along the streak, turned about its middle.
        let thick = 0.8 + 1.6 * depth;
        let len = f64::from(dx).hypot(f64::from(dy)) as f32 + 1.0;
        let angle = (-dy).atan2(-dx);
        let (cx, cy) = (x + angle.cos() * len / 2.0, y + angle.sin() * len / 2.0);
        node.left = px(cx - len / 2.0);
        node.top = px(cy - thick / 2.0);
        node.width = px(len);
        node.height = px(thick);
        ui.rotation = Rot2::radians(angle);
        colour.0 = shade.to_bevy();
    }
}

/// The real clock's hour, for the example's capture names.
pub fn held_hour(clock: &Clock) -> Option<f32> {
    (clock.hour >= 0.0).then_some(clock.hour)
}

/// A pixel of a dome painted for `paint`, for checking a capture against
/// the texture (the way `dir`).
pub fn dome_pixel(paint: &SkyPaint, dir: Vec3) -> Rgba {
    super::skytex::sky_at(dir, paint.dir, paint.dusk, paint.night, paint.turn)
}

#[allow(dead_code)]
fn pixel_direction(x: usize, y: usize) -> Vec3 {
    pixel_dir(x, y, SKY_TEX_W, SKY_TEX_H)
}
