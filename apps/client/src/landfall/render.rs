//! The viewer's side of the subsystem: the meshes and paint the headless
//! decisions call for. The terrain's chunks and tiles are shaped and
//! painted here (`shape`, `paint` and `writeTileHeights` in terrain.go),
//! the scatter's models follow their slots, the pieces' bounds are read
//! off their loaded models for the cull, [`Drawn`] becomes visibility and
//! render layers, and the layout's small pieces are merged
//! (`PlaceMerged`).
//!
//! Ground carries `shading::Smooth`; streaming supports both the initial
//! standard material and the painted material installed by the shading plugin.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    camera::{primitives::MeshAabb, visibility::RenderLayers},
    image::{Image, ImageSampler},
    light::NotShadowCaster,
    math::Affine3A,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
    world_serialization::WorldInstanceReady,
};
use earth_two_world::{
    kit::quarter_turns,
    terrain::{
        CHUNK_CELLS, CHUNK_SIZE, GROUND_LEVEL, TILE_SIZE, drawn_height, ground_colour, sample,
    },
};

use super::{
    Budget, LandfallSet,
    cull::{CullEye, Drawn, PieceBounds, Sphere},
    merge::{self, Look, MergePlan, MergeRequest, MeshSource, PieceMesh},
    scatter::ScatterModel,
    terrain::{
        ChunkRebuild, Terrain, TerrainChunk, TerrainTile, TileResink, no_sink, sink_under,
        tile_heights, write_tile_heights,
    },
};
use crate::world::{KitAsset, PieceModel, Placed};

/// The render layer of what's drawn to the camera, and of what's drawn
/// only into the shadow map. The camera sees the first; the sun must light
/// both (the shading port puts it on `RenderLayers::from_layers(&[0, 1])`).
pub const SEEN_LAYER: usize = 0;
pub const SHADOW_LAYER: usize = 1;

/// A tile's unsunk heights, once per grid point: the cache
/// `writeTileHeights` masks from.
#[derive(Component, Debug, Clone, Default)]
pub struct TileHeights(pub Vec<f32>);

/// A piece whose model has loaded: its bounds are to be read for the cull.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BoundsPending;

/// Each model's bounds, by its path, read once.
#[derive(Resource, Debug, Default)]
pub struct ModelBounds(pub HashMap<String, Sphere>);

pub struct LandfallRenderPlugin;

impl Plugin for LandfallRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModelBounds>()
            .add_observer(model_ready)
            .add_systems(
                Update,
                (
                    eye_from_camera,
                    build_terrain,
                    rebuild_chunks,
                    resink_tiles,
                    follow_scatter_models,
                    read_bounds,
                    merge_pieces,
                )
                    .chain()
                    .after(LandfallSet::Stream)
                    .before(LandfallSet::Cull),
            )
            .add_systems(Update, apply_drawn.after(LandfallSet::Cull));
    }
}

/// The camera's view as the cull wants it: its vertical field of view and
/// the window's aspect.
fn eye_from_camera(
    mut commands: Commands,
    cameras: Query<(Entity, &Projection, Option<&CullEye>), With<Camera3d>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let aspect = windows
        .single()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(16.0 / 9.0);
    for (entity, projection, had) in &cameras {
        let fovy = match projection {
            Projection::Perspective(p) => p.fov.to_degrees(),
            _ => 45.0,
        };
        let eye = CullEye { fovy, aspect };
        if had != Some(&eye) {
            commands.entity(entity).insert(eye);
        }
    }
}

/// `shape`: a square of ground `size` across, `cells` a side, round (cx,
/// cz), in its own frame: its vertices from `height`, sunk by `sink`, and
/// their normals. Vertices are shared between neighbouring triangles; a
/// grid point's noise and normal are evaluated once.
pub fn terrain_mesh(
    cx: f32,
    cz: f32,
    size: f32,
    cells: usize,
    height: impl Fn(f32, f32) -> f32,
    sink: impl Fn(f32, f32) -> f32,
) -> (Mesh, Vec<f32>) {
    let samples = sample(cx, cz, size, cells, height);
    let n = cells + 1;
    let step = size / cells as f32;
    let mut positions = Vec::with_capacity(n * n);
    let mut normals = Vec::with_capacity(n * n);
    let mut uvs = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let (x, z) = (-size / 2.0 + i as f32 * step, -size / 2.0 + j as f32 * step);
            let s = samples[j * n + i];
            positions.push([x, GROUND_LEVEL + s.height - sink(cx + x, cz + z), z]);
            normals.push([s.normal.x, s.normal.y, s.normal.z]);
            // raylib's plane runs its texture along X and down Z.
            uvs.push([i as f32 / cells as f32, j as f32 / cells as f32]);
        }
    }
    let mut indices = Vec::with_capacity(cells * cells * 6);
    for j in 0..cells {
        for i in 0..cells {
            let (a, b) = ((j * n + i) as u32, ((j + 1) * n + i) as u32);
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    (mesh, tile_heights(&samples))
}

/// `paint`: a texture of the ground round (cx, cz), `size` across,
/// `texels` a side, bilinear.
pub fn paint(cx: f32, cz: f32, size: f32, texels: usize) -> Image {
    let mut pixels = Vec::with_capacity(texels * texels * 4);
    for y in 0..texels {
        for x in 0..texels {
            let wx = cx - size / 2.0 + (x as f32 + 0.5) * size / texels as f32;
            let wz = cz - size / 2.0 + (y as f32 + 0.5) * size / texels as f32;
            let [r, g, b] = ground_colour(wx, wz);
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: texels as u32,
            height: texels as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Initial matte ground material, adopted by the painted shader when enabled.
fn ground_material(texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(texture),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    }
}

/// Shapes and paints the chunks and tiles the headless spawn made. The
/// ground takes shadows but casts none.
#[allow(clippy::too_many_arguments)]
fn build_terrain(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    budget: Res<Budget>,
    chunks: Query<(Entity, &Transform), Added<TerrainChunk>>,
    tiles: Query<(Entity, &Transform), Added<TerrainTile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let sink = sink_under(terrain.window());
    for (entity, tr) in &tiles {
        let (cx, cz) = (tr.translation.x, tr.translation.z);
        let (mesh, heights) =
            terrain_mesh(cx, cz, TILE_SIZE, budget.tile_cells, drawn_height, sink);
        let texture = images.add(paint(cx, cz, TILE_SIZE, budget.tile_texels));
        commands.entity(entity).insert((
            Mesh3d(meshes.add(mesh)),
            crate::shading::Smooth,
            MeshMaterial3d(materials.add(ground_material(texture))),
            TileHeights(heights),
            NotShadowCaster,
            Visibility::default(),
        ));
    }
    for (entity, tr) in &chunks {
        let (cx, cz) = (tr.translation.x, tr.translation.z);
        let (mesh, _) = terrain_mesh(cx, cz, CHUNK_SIZE, CHUNK_CELLS, drawn_height, no_sink);
        let texture = images.add(paint(cx, cz, CHUNK_SIZE, budget.chunk_texels));
        commands.entity(entity).insert((
            Mesh3d(meshes.add(mesh)),
            crate::shading::Smooth,
            MeshMaterial3d(materials.add(ground_material(texture))),
            NotShadowCaster,
            Visibility::default(),
        ));
    }
}

/// Reshapes and repaints the chunks the stream handed another chunk, where
/// their transforms now are.
#[allow(clippy::type_complexity)]
fn rebuild_chunks(
    mut commands: Commands,
    budget: Res<Budget>,
    chunks: Query<
        (
            Entity,
            &Transform,
            &Mesh3d,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<crate::shading::PaintedMaterial>>,
        ),
        With<ChunkRebuild>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut painted: Option<ResMut<Assets<crate::shading::PaintedMaterial>>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, tr, mesh, standard, painted_handle) in &chunks {
        let (cx, cz) = (tr.translation.x, tr.translation.z);
        let (shaped, _) = terrain_mesh(cx, cz, CHUNK_SIZE, CHUNK_CELLS, drawn_height, no_sink);
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            *m = shaped;
        }
        let texture = images.add(paint(cx, cz, CHUNK_SIZE, budget.chunk_texels));
        let old = if let Some(mut m) = standard.and_then(|h| materials.get_mut(&h.0)) {
            m.base_color_texture.replace(texture)
        } else if let Some(mut m) = painted_handle.and_then(|h| painted.as_mut()?.get_mut(&h.0)) {
            m.base.base_color_texture.replace(texture)
        } else {
            images.remove(&texture);
            continue;
        };
        if let Some(old) = old {
            images.remove(&old);
        }
        commands.entity(entity).remove::<ChunkRebuild>();
    }
}

/// `resink`: writes the tiles' heights again with the window's sink, from
/// their cached heights; no noise or normal rebuild.
fn resink_tiles(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    budget: Res<Budget>,
    tiles: Query<(Entity, &Transform, &TileHeights, &Mesh3d), With<TileResink>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let sink = sink_under(terrain.window());
    for (entity, tr, heights, mesh) in &tiles {
        if let Some(mut m) = meshes.get_mut(&mesh.0)
            && let Some(VertexAttributeValues::Float32x3(positions)) =
                m.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            write_tile_heights(
                positions.as_flattened_mut(),
                tr.translation.x,
                tr.translation.z,
                TILE_SIZE,
                budget.tile_cells,
                &heights.0,
                sink,
            );
        }
        commands.entity(entity).remove::<TileResink>();
    }
}

/// A scatter slot handed another cell shows the cell's piece: its model
/// child is replaced (the viewer attaches new `PieceModel`s itself).
#[allow(clippy::type_complexity)]
fn follow_scatter_models(
    mut commands: Commands,
    assets: Res<AssetServer>,
    changed: Query<
        (Entity, &PieceModel, Option<&Children>),
        (With<ScatterModel>, Changed<PieceModel>),
    >,
    roots: Query<(), With<WorldAssetRoot>>,
) {
    for (entity, model, children) in &changed {
        let Some(children) = children else {
            continue; // just spawned: `attach_pieces` gives it its model
        };
        for child in children.iter() {
            if roots.contains(child) {
                commands.entity(child).despawn();
            }
        }
        let scene: Handle<WorldAsset> = assets.load(format!("{}#Scene0", model.0));
        commands
            .entity(entity)
            .with_child((Name::new("model"), WorldAssetRoot(scene)));
    }
}

/// A loaded model under a placed piece or a scatter thing: its bounds are
/// read next frame, once its transforms have propagated.
#[allow(clippy::type_complexity)]
fn model_ready(
    event: On<WorldInstanceReady>,
    parents: Query<&ChildOf>,
    pieces: Query<(), Or<(With<Placed>, With<ScatterModel>)>>,
    mut commands: Commands,
) {
    if let Ok(parent) = parents.get(event.entity)
        && pieces.contains(parent.parent())
    {
        commands.entity(parent.parent()).insert(BoundsPending);
    }
}

/// The box round the meshes under `root`, in `root`'s frame.
fn model_box(
    root: Entity,
    root_global: &GlobalTransform,
    children: &Query<&Children>,
    meshes: &Query<(&Mesh3d, &GlobalTransform)>,
    assets: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let to_root = root_global.affine().inverse();
    let mut lo = Vec3::MAX;
    let mut hi = Vec3::MIN;
    let mut any = false;
    for e in children.iter_descendants(root) {
        let Ok((mesh, global)) = meshes.get(e) else {
            continue;
        };
        let Some(aabb) = assets.get(&mesh.0).and_then(|m| m.get_aabb()) else {
            continue;
        };
        let local = to_root * global.affine();
        let (min, max) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
        for corner in 0..8 {
            let c = Vec3::new(
                if corner & 1 == 0 { min.x } else { max.x },
                if corner & 2 == 0 { min.y } else { max.y },
                if corner & 4 == 0 { min.z } else { max.z },
            );
            let p = local.transform_point3(c);
            lo = lo.min(p);
            hi = hi.max(p);
            any = true;
        }
    }
    any.then_some((lo, hi))
}

/// `cull`'s bounds: each model's bounding box as a sphere about its
/// middle, read once per model and kept by path.
#[allow(clippy::type_complexity)]
fn read_bounds(
    mut commands: Commands,
    mut known: ResMut<ModelBounds>,
    pending: Query<(Entity, &PieceModel, &GlobalTransform), With<BoundsPending>>,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, &GlobalTransform)>,
    assets: Res<Assets<Mesh>>,
) {
    for (entity, model, global) in &pending {
        let sphere = match known.0.get(&model.0) {
            Some(s) => *s,
            None => {
                let Some((lo, hi)) = model_box(entity, global, &children, &meshes, &assets) else {
                    continue; // meshes still arriving
                };
                let s = Sphere {
                    center: (lo + hi) * 0.5,
                    radius: lo.distance(hi) / 2.0,
                };
                known.0.insert(model.0.clone(), s);
                s
            }
        };
        commands
            .entity(entity)
            .insert(PieceBounds(sphere))
            .remove::<BoundsPending>();
    }
}

/// The renderer's side of [`Drawn`]: seen pieces on the camera's layer,
/// shadow-only ones on the shadow layer alone, unseen ones hidden.
fn apply_drawn(mut commands: Commands, changed: Query<(Entity, &Drawn), Changed<Drawn>>) {
    for (entity, drawn) in &changed {
        match drawn {
            Drawn::Seen => commands
                .entity(entity)
                .insert((Visibility::Inherited, RenderLayers::layer(SEEN_LAYER))),
            Drawn::ShadowOnly => commands
                .entity(entity)
                .insert((Visibility::Inherited, RenderLayers::layer(SHADOW_LAYER))),
            Drawn::Unseen => commands.entity(entity).insert(Visibility::Hidden),
        };
    }
}

/// A material's look, for merging: its texture's id, its colour and glow,
/// and whether it's opaque.
fn look_of(material: &StandardMaterial) -> (Look, bool) {
    use std::hash::{Hash, Hasher};
    let texture = material
        .base_color_texture
        .as_ref()
        .map(|h| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            h.id().hash(&mut hasher);
            hasher.finish().max(1)
        })
        .unwrap_or(0);
    let bytes = |c: Color| {
        let s = c.to_srgba();
        [
            (s.red * 255.0).round() as u8,
            (s.green * 255.0).round() as u8,
            (s.blue * 255.0).round() as u8,
            (s.alpha * 255.0).round() as u8,
        ]
    };
    let emissive = if material.emissive_texture.is_some() {
        bytes(material.emissive.into())
    } else {
        [0, 0, 0, 0]
    };
    (
        Look {
            texture,
            emissive,
            color: bytes(material.base_color),
        },
        matches!(material.alpha_mode, AlphaMode::Opaque),
    )
}

/// A loaded mesh as the plan takes it.
fn source_of(mesh: &Mesh, to_piece: &Affine3A, turn_back: Quat) -> Option<MeshSource> {
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(p) => p
            .iter()
            .map(|p| to_piece.transform_point3(Vec3::from(*p)))
            .collect(),
        _ => return None,
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) => Some(
            n.iter()
                .map(|n| (turn_back * Vec3::from(*n)).normalize_or_zero())
                .collect(),
        ),
        _ => None,
    };
    let texcoords = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(uv)) => {
            Some(uv.iter().map(|uv| Vec2::from(*uv)).collect())
        }
        _ => None,
    };
    let indices = mesh.indices().map(|i| i.iter().map(|i| i as u32).collect());
    Some(MeshSource {
        positions,
        normals,
        texcoords,
        indices,
        skinned: mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX).is_some(),
    })
}

/// `PlaceMerged`: once every placed piece's model has loaded, merges the
/// small ones by square and look, hides their own models, and spawns the
/// merged meshes and shadows in a fixed order.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn merge_pieces(
    mut commands: Commands,
    request: Option<Res<MergeRequest>>,
    kits: Res<Assets<KitAsset>>,
    placed: Query<(Entity, &Placed, &PieceModel, &GlobalTransform, &Children)>,
    pending: Query<(), With<BoundsPending>>,
    bounds: Query<&PieceBounds>,
    children: Query<&Children>,
    roots: Query<(), With<WorldAssetRoot>>,
    parents: Query<&ChildOf>,
    layouts: Query<&crate::world::LayoutRoot>,
    mesh_parts: Query<(
        &Mesh3d,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&crate::shading::SourceMaterial>,
        &GlobalTransform,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut new_materials: Local<Vec<(Look, Handle<StandardMaterial>)>>,
) {
    let Some(request) = request else {
        return;
    };
    let Some(kit) = kits.iter().map(|(_, k)| &k.0).next() else {
        return;
    };
    // Every piece's model must be in, so the merge is of the whole layout.
    let mut pieces: Vec<_> = placed
        .iter()
        .filter(|(e, ..)| {
            parents
                .get(*e)
                .ok()
                .and_then(|p| layouts.get(p.parent()).ok())
                .is_some_and(|root| root.layout == "world/landfall.json")
        })
        .collect();
    if pieces.len() < request.placements.len() || pieces.iter().any(|(e, ..)| pending.contains(*e))
    {
        return;
    }
    if pieces.iter().any(|(e, ..)| !bounds.contains(*e)) {
        return;
    }
    let mut plan = MergePlan::default();
    let mut small: HashMap<String, (f32, bool)> = HashMap::new();
    pieces.sort_by_key(|(_, p, ..)| p.index);
    for (entity, placed, model, global, kids) in pieces {
        let Some(placement) = request.placements.get(placed.index) else {
            continue;
        };
        let piece = kit.pieces.get(&placement.piece);
        if piece.is_none_or(|p| p.vehicle.is_some()) {
            continue;
        }
        let turn = quarter_turns(placement.turns);
        let to_piece = global.affine().inverse();
        let mut parts: Vec<PieceMesh> = Vec::new();
        let mut sources = Vec::new();
        for e in children.iter_descendants(entity) {
            let Ok((mesh, material, source_material, part_global)) = mesh_parts.get(e) else {
                continue;
            };
            let handle = material
                .map(|m| &m.0)
                .or_else(|| source_material.map(|m| &m.0));
            let (Some(m), Some(mat)) = (meshes.get(&mesh.0), handle.and_then(|h| materials.get(h)))
            else {
                continue;
            };
            let local = to_piece * part_global.affine();
            let (_, rot, _) = local.to_scale_rotation_translation();
            let Some(source) = source_of(m, &local, rot) else {
                continue;
            };
            let (look, opaque) = look_of(mat);
            sources.push(source.clone());
            parts.push(PieceMesh {
                source,
                look,
                opaque,
            });
        }
        let (radius, mergeable) = *small
            .entry(model.0.clone())
            .or_insert_with(|| merge::mergeable(&sources));
        if !mergeable {
            continue;
        }
        plan.add_piece(placement.at(), turn, radius, &parts);
        // Drawn merged: its own model is hidden, its parts still placed.
        for child in kids.iter() {
            if roots.contains(child) {
                commands.entity(child).insert(Visibility::Hidden);
            }
        }
    }
    let mut spawned = 0;
    for merged in plan.finish() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            merged
                .data
                .positions
                .iter()
                .map(|v| v.to_array())
                .collect::<Vec<_>>(),
        )
        .with_inserted_indices(Indices::U16(merged.data.indices.clone()));
        if merged.look.is_some() {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                merged
                    .data
                    .normals
                    .iter()
                    .map(|v| v.to_array())
                    .collect::<Vec<_>>(),
            );
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_UV_0,
                merged
                    .data
                    .texcoords
                    .iter()
                    .map(|v| v.to_array())
                    .collect::<Vec<_>>(),
            );
        }
        let transform = Transform::from_translation(merge::cell_origin(merged.cell));
        let mut e = commands.spawn((
            Name::new("merged"),
            Mesh3d(meshes.add(mesh)),
            transform,
            merged.merged,
            Visibility::default(),
        ));
        match merged.look {
            Some(look) => {
                // The piece's own material, shared between the merged
                // meshes of a look: the first piece with it lends its handle.
                let handle = new_materials
                    .iter()
                    .find(|(l, _)| *l == look)
                    .map(|(_, h)| h.clone())
                    .or_else(|| {
                        material_with_look(&materials, &placed, &children, &mesh_parts, look)
                    });
                if let Some(handle) = handle {
                    new_materials.push((look, handle.clone()));
                    e.insert(MeshMaterial3d(handle));
                }
                if !merged.merged.casts {
                    e.insert(NotShadowCaster);
                }
            }
            None => {
                e.insert(RenderLayers::layer(SHADOW_LAYER));
            }
        }
        spawned += 1;
    }
    info!("landfall: merged the layout into {spawned} meshes");
    commands.remove_resource::<MergeRequest>();
}

/// The first placed material with `look`: the merged mesh is painted with
/// its pieces' own texture.
#[allow(clippy::type_complexity)]
fn material_with_look(
    materials: &Assets<StandardMaterial>,
    placed: &Query<(Entity, &Placed, &PieceModel, &GlobalTransform, &Children)>,
    children: &Query<&Children>,
    mesh_parts: &Query<(
        &Mesh3d,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&crate::shading::SourceMaterial>,
        &GlobalTransform,
    )>,
    look: Look,
) -> Option<Handle<StandardMaterial>> {
    for (entity, ..) in placed.iter() {
        for e in children.iter_descendants(entity) {
            if let Ok((_, material, source_material, _)) = mesh_parts.get(e)
                && let Some(handle) = material
                    .map(|m| &m.0)
                    .or_else(|| source_material.map(|m| &m.0))
                && let Some(mat) = materials.get(handle)
                && look_of(mat).0 == look
            {
                return Some(handle.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod streaming_tests {
    use super::*;

    #[test]
    fn relocated_chunks_rebuild_before_and_after_painted_material_adoption() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<crate::shading::PaintedMaterial>>()
            .insert_resource(Budget::BROWSER)
            .add_systems(Update, rebuild_chunks);
        for painted in [false, true] {
            let mesh = app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(terrain_mesh(0., 0., CHUNK_SIZE, CHUNK_CELLS, drawn_height, no_sink).0);
            let image = app
                .world_mut()
                .resource_mut::<Assets<Image>>()
                .add(paint(0., 0., CHUNK_SIZE, 4));
            let original_image = image.id();
            let entity = app
                .world_mut()
                .spawn((
                    ChunkRebuild,
                    Transform::from_xyz(9920., 0., -1760.),
                    Mesh3d(mesh.clone()),
                ))
                .id();
            if painted {
                let material = app
                    .world_mut()
                    .resource_mut::<Assets<crate::shading::PaintedMaterial>>()
                    .add(crate::shading::PaintedMaterial {
                        base: ground_material(image),
                        extension: default(),
                    });
                app.world_mut()
                    .entity_mut(entity)
                    .insert(MeshMaterial3d(material));
            } else {
                let material = app
                    .world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(ground_material(image));
                app.world_mut()
                    .entity_mut(entity)
                    .insert(MeshMaterial3d(material));
            }
            app.update();
            assert!(app.world().get::<ChunkRebuild>(entity).is_none());
            assert!(
                !app.world()
                    .resource::<Assets<Image>>()
                    .contains(original_image)
            );
            let actual = app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap();
            let expected = terrain_mesh(
                9920.,
                -1760.,
                CHUNK_SIZE,
                CHUNK_CELLS,
                drawn_height,
                no_sink,
            )
            .0;
            assert_eq!(
                actual.attribute(Mesh::ATTRIBUTE_POSITION),
                expected.attribute(Mesh::ATTRIBUTE_POSITION)
            );
        }
    }
}
