//! The GPU half: loading the character GLBs, spawning their scenes under
//! bodies and garments, assembling the skin (the tone texture on the skin
//! meshes, the hidden regions under clothes), handing the clips to Bevy's
//! animation, and on desktop writing the cloth solver's vertices back into
//! the meshes each frame.

use bevy::animation::AnimationPlayer as BevyPlayer;
use bevy::asset::LoadState;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::{Gltf, GltfNode, GltfSkin};
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::world_serialization::WorldAssetRoot;
use std::collections::{HashMap, HashSet};

use super::bones::Skeleton;
use super::graph::{Animated, Graphs};
use super::mesh::{ModelMeshes, ModelStore, SkinnedMeshData};
use super::outfit::{ModelParts, ModelPath, Slot, load_wardrobe};
use super::player::{ClipLibrary, Libraries};
use super::roster::{BODY_HEIGHT, Model, Roster, Skin};
use super::verlet::{Cloth, simulate};
use super::{Wardrobe, paused};
use crate::character::Controls;
use crate::shading::Outlined;

/// Content is the plugin's configuration: the models to load and the
/// wardrobe file's text (the viewer reads it from the asset root; a browser
/// shell may hand it over another way). Insert it before the plugin's
/// startup runs.
#[derive(Resource, Clone, Debug, Default)]
pub struct Content {
    pub models: Vec<Model>,
    pub wardrobe: Option<String>,
}

impl Content {
    /// earth_two is the game's people and wardrobe, read from asset_root.
    pub fn earth_two(asset_root: &str) -> Content {
        let path = std::path::Path::new(asset_root).join(super::content::WARDROBE);
        Content {
            models: super::content::people(),
            wardrobe: std::fs::read_to_string(&path).ok(),
        }
    }
}

/// Loading is a resource: every GLB the roster and wardrobe name, by path,
/// and which of them have been indexed.
#[derive(Resource, Default)]
pub struct Loading {
    pub gltfs: HashMap<String, Handle<Gltf>>,
    indexed: HashSet<String>,
}

impl Loading {
    pub fn handle(&self, path: &str) -> Option<&Handle<Gltf>> {
        self.gltfs.get(path)
    }
}

/// ModelScene marks a ModelPath entity's spawned scene child, for the path
/// it was spawned from.
#[derive(Component, Clone, Debug)]
pub struct ModelScene {
    pub path: String,
    pub root: Entity,
}

/// MeshEntities maps a model's mesh indices to the entities drawing them,
/// once its scene is in.
#[derive(Component, Clone, Debug, Default)]
pub struct MeshEntities(pub Vec<Option<Entity>>);

/// ClothMeshes marks the mesh entities a Cloth writes into, with the
/// private copies of their meshes.
#[derive(Component, Clone, Debug, Default)]
pub struct ClothMeshes(pub Vec<(usize, Entity, Handle<Mesh>)>);

pub struct ViewerPlugin;

impl Plugin for ViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loading>()
            .add_systems(
                Update,
                load_content.run_if(
                    bevy::ecs::schedule::common_conditions::resource_exists_and_changed::<Content>,
                ),
            )
            .add_systems(
                Update,
                (index_models, spawn_scenes, wire_scenes, apply_parts).chain(),
            )
            .add_systems(
                PostUpdate,
                simulate_cloth
                    .after(TransformSystems::Propagate)
                    .after(bevy::app::AnimationSystems),
            );
    }
}

/// load_content starts the GLBs loading and builds the roster and the
/// wardrobe from Content, as the Go plugin's PreStartup did.
fn load_content(
    mut commands: Commands,
    content: Option<Res<Content>>,
    assets: Res<AssetServer>,
    mut loading: ResMut<Loading>,
) {
    let Some(content) = content else { return };
    let mut roster = Roster::default();
    for m in &content.models {
        roster
            .skins
            .push(Skin::new(&m.path, m.clips.clone(), m.scale));
        loading
            .gltfs
            .entry(m.path.clone())
            .or_insert_with(|| assets.load(m.path.clone()));
    }
    let mut wardrobe = Wardrobe {
        bodies: vec![Default::default(); content.models.len()],
    };
    if let Some(text) = &content.wardrobe {
        // Mesh counts come once the models are in (see index_models).
        match load_wardrobe(text, &content.models, |_| None) {
            Ok(w) => wardrobe = w,
            Err(e) => error!("{e}"),
        }
        for b in &wardrobe.bodies {
            for slot in Slot::ALL {
                for item in b.items(slot) {
                    loading
                        .gltfs
                        .entry(item.model.clone())
                        .or_insert_with(|| assets.load(item.model.clone()));
                }
            }
        }
    }
    commands.insert_resource(roster);
    commands.insert_resource(wardrobe);
}

/// index_models reads each GLB as it finishes loading: its clips into the
/// library and graph, its geometry and skeleton into the model store, and
/// a scale for any skin that needs fitting to BODY_HEIGHT.
#[allow(clippy::too_many_arguments)]
fn index_models(
    assets: Res<AssetServer>,
    mut loading: ResMut<Loading>,
    gltfs: Res<Assets<Gltf>>,
    meshes: Res<Assets<Mesh>>,
    nodes: Res<Assets<GltfNode>>,
    skins: Res<Assets<GltfSkin>>,
    binds: Res<Assets<SkinnedMeshInverseBindposes>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut libraries: ResMut<Libraries>,
    mut clip_graphs: ResMut<Graphs>,
    mut store: ResMut<ModelStore>,
    mut roster: ResMut<Roster>,
    mut wardrobe: ResMut<Wardrobe>,
) {
    let pending: Vec<(String, Handle<Gltf>)> = loading
        .gltfs
        .iter()
        .filter(|(path, _)| !loading.indexed.contains(*path))
        .map(|(p, h)| (p.clone(), h.clone()))
        .collect();
    for (path, handle) in pending {
        if matches!(assets.load_state(handle.id()), LoadState::Failed(_)) {
            error!("presentation: {path} failed to load");
            loading.indexed.insert(path);
            continue;
        }
        if !assets.is_loaded_with_dependencies(handle.id()) {
            continue;
        }
        let Some(gltf) = gltfs.get(&handle) else {
            continue;
        };
        let Some(model) = read_model(gltf, &meshes, &nodes, &skins, &binds, &gltf_meshes) else {
            continue;
        };
        if !gltf.named_animations.is_empty() {
            libraries.by_model.insert(
                path.clone(),
                ClipLibrary::new(gltf.named_animations.iter().map(|(name, h)| {
                    (name.to_string(), clips.get(h).map_or(0.0, |c| c.duration()))
                })),
            );
            clip_graphs.build(
                &path,
                gltf.named_animations
                    .iter()
                    .map(|(name, h)| (name.to_string(), h.clone())),
                &mut graphs,
            );
        }
        for skin in roster
            .skins
            .iter_mut()
            .filter(|s| s.model == path && s.scale == 0.0)
        {
            // The bind pose's height; models are rigged standing up.
            skin.scale = 1.0;
            let (lo, hi) = model
                .meshes
                .iter()
                .flat_map(|m| &m.positions)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p.y), hi.max(p.y))
                });
            if hi - lo > 0.0 {
                skin.scale = BODY_HEIGHT / (hi - lo);
            }
        }
        // Only a face's skin is outlined, not its eyes and eyebrows.
        for b in &mut wardrobe.bodies {
            for item in b
                .items_mut(Slot::Face)
                .iter_mut()
                .filter(|it| it.model == path)
            {
                item.unlined = (0..model.meshes.len())
                    .filter(|m| !item.skin_meshes.contains(m))
                    .collect();
            }
        }
        store.insert(&path, model);
        loading.indexed.insert(path);
    }
}

/// read_model takes a loaded GLB's meshes (in file order, one primitive
/// each, as raylib numbered them) and its skin's skeleton.
fn read_model(
    gltf: &Gltf,
    meshes: &Assets<Mesh>,
    nodes: &Assets<GltfNode>,
    skins: &Assets<GltfSkin>,
    binds: &Assets<SkinnedMeshInverseBindposes>,
    gltf_meshes: &Assets<bevy::gltf::GltfMesh>,
) -> Option<ModelMeshes> {
    let mut model = ModelMeshes::default();
    for gm in &gltf.meshes {
        let gm = gltf_meshes.get(gm)?;
        for prim in &gm.primitives {
            let mesh = meshes.get(&prim.mesh)?;
            model.meshes.push(read_mesh(mesh));
        }
    }
    if let Some(skin) = gltf.skins.first().and_then(|h| skins.get(h)) {
        let ibm = binds.get(&skin.inverse_bind_matrices)?;
        let joints: Vec<&GltfNode> = skin
            .joints
            .iter()
            .map(|h| nodes.get(h))
            .collect::<Option<_>>()?;
        let index: HashMap<usize, usize> = joints
            .iter()
            .enumerate()
            .map(|(i, n)| (n.index, i))
            .collect();
        let mut parents = vec![-1i32; joints.len()];
        for (i, j) in joints.iter().enumerate() {
            for child in &j.children {
                if let Some(c) = nodes.get(child)
                    && let Some(&ci) = index.get(&c.index)
                {
                    parents[ci] = i as i32;
                }
            }
        }
        model.skeleton = Skeleton {
            names: joints.iter().map(|n| n.name.clone()).collect(),
            parents,
            bind: ibm
                .iter()
                .map(|m| Transform::from_matrix(m.inverse()))
                .collect(),
        };
    }
    Some(model)
}

fn read_mesh(mesh: &Mesh) -> SkinnedMeshData {
    let mut m = SkinnedMeshData::default();
    if let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        m.positions = p.iter().map(|v| Vec3::from_array(*v)).collect();
    }
    if let Some(VertexAttributeValues::Float32x3(n)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        m.normals = n.iter().map(|v| Vec3::from_array(*v)).collect();
    }
    if let Some(VertexAttributeValues::Uint16x4(j)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
        m.joints = j.clone();
    }
    if let Some(VertexAttributeValues::Float32x4(w)) = mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
    {
        m.weights = w.clone();
    }
    if let Some(indices) = mesh.indices() {
        m.indices = indices.iter().map(|i| i as u32).collect();
    }
    m
}

/// spawn_scenes puts each ModelPath entity's scene under it, and swaps it
/// when the path changes (a body dressed in another skin).
fn spawn_scenes(
    mut commands: Commands,
    assets: Res<AssetServer>,
    loading: Res<Loading>,
    gltfs: Res<Assets<Gltf>>,
    wanted: Query<(Entity, &ModelPath, Option<&ModelScene>)>,
) {
    for (e, path, scene) in &wanted {
        if scene.is_some_and(|s| s.path == path.0) {
            continue;
        }
        let Some(handle) = loading.handle(&path.0) else {
            continue;
        };
        if !assets.is_loaded_with_dependencies(handle.id()) {
            continue;
        }
        let Some(gltf) = gltfs.get(handle) else {
            continue;
        };
        let Some(first) = gltf.scenes.first() else {
            continue;
        };
        if let Some(old) = scene {
            commands.entity(old.root).despawn();
        }
        let root = commands
            .spawn((
                Name::new("model"),
                WorldAssetRoot(first.clone()),
                ChildOf(e),
            ))
            .id();
        commands
            .entity(e)
            .insert((
                ModelScene {
                    path: path.0.clone(),
                    root,
                },
                Visibility::default(),
            ))
            .remove::<(Animated, MeshEntities, ClothMeshes)>();
    }
}

/// wire_scenes finds, once a scene is in, the entity Bevy animates and the
/// entities drawing each mesh, so the clip clock and the model parts can
/// reach them.
fn wire_scenes(
    mut commands: Commands,
    assets: Res<AssetServer>,
    scenes: Query<(Entity, &ModelScene), Without<Animated>>,
    children: Query<&Children>,
    players: Query<(), With<BevyPlayer>>,
    drawn: Query<&Mesh3d>,
    store: Res<ModelStore>,
) {
    for (e, scene) in &scenes {
        let Some(model) = store.get(&scene.path) else {
            continue;
        };
        let mut animated = None;
        let mut meshes = vec![None; model.meshes.len()];
        for d in children.iter_descendants(scene.root) {
            if animated.is_none() && players.contains(d) {
                animated = Some(d);
            }
            if let Ok(mesh) = drawn.get(d)
                && let Some(path) = assets.get_path(mesh.0.id())
                && let Some(i) = mesh_index(path.label().unwrap_or(""))
                && i < meshes.len()
            {
                meshes[i] = Some(d);
            }
        }
        let Some(animated) = animated else { continue };
        commands
            .entity(e)
            .insert((Animated(animated), MeshEntities(meshes)));
    }
}

/// mesh_index reads raylib's mesh index from a primitive's asset label,
/// "Mesh{i}/Primitive{j}": each of the character GLBs' meshes has one
/// primitive, so it's the mesh's.
pub fn mesh_index(label: &str) -> Option<usize> {
    label.strip_prefix("Mesh")?.split('/').next()?.parse().ok()
}

/// apply_parts hides the meshes ModelParts hides and gives the skin
/// meshes the tone texture, on their own copy of the material. Bodies and
/// clothes are outlined, but for hair and glasses (which carry no
/// OutlineSkip).
#[allow(clippy::type_complexity)]
fn apply_parts(
    mut commands: Commands,
    assets: Res<AssetServer>,
    parts: Query<
        (
            Entity,
            &ModelParts,
            &MeshEntities,
            Option<&super::outfit::OutlineSkip>,
        ),
        Or<(Changed<ModelParts>, Changed<MeshEntities>)>,
    >,
    drawn: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut toned: Local<HashMap<(Entity, usize), Handle<StandardMaterial>>>,
) {
    for (e, p, meshes, outline) in &parts {
        for (i, mesh) in meshes.0.iter().enumerate() {
            let Some(mesh) = *mesh else { continue };
            let mut entity = commands.entity(mesh);
            entity.insert(if p.hidden.contains(&i) {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
            if let Some(tone) = p.texture.get(&i)
                && let Ok(material) = drawn.get(mesh)
                && let Some(own) = materials.get(&material.0).cloned()
            {
                let handle = toned.entry((e, i)).or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color_texture: Some(assets.load(tone.clone())),
                        ..own
                    })
                });
                entity.insert(MeshMaterial3d(handle.clone()));
            }
        }
        if outline.is_some() {
            commands.entity(e).insert(Outlined);
        }
    }
}

/// simulate_cloth runs the solver on each clothed garment after the pose is
/// in: it skins the garment's meshes itself from the joints' transforms,
/// moves the loose vertices, and writes positions and normals back into
/// the garment's own copies of the meshes, drawn without Bevy's skinning.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn simulate_cloth(
    mut commands: Commands,
    mut garments: Query<(
        Entity,
        &mut Cloth,
        &ModelScene,
        &MeshEntities,
        Option<&mut ClothMeshes>,
    )>,
    skinned: Query<(&Mesh3d, &SkinnedMesh, &GlobalTransform)>,
    transforms: Query<&GlobalTransform>,
    binds: Res<Assets<SkinnedMeshInverseBindposes>>,
    mut meshes: ResMut<Assets<Mesh>>,
    store: Res<ModelStore>,
    controls: Res<Controls>,
    physics: Option<Res<Time<avian3d::prelude::Physics>>>,
    time: Res<Time>,
) {
    if !controls.enabled || paused(physics.as_deref()) {
        return;
    }
    let dt = time.delta_secs();
    for (e, mut cloth, scene, drawn, cloth_meshes) in &mut garments {
        let Some(model) = store.get(&scene.path) else {
            continue;
        };
        // The joints from any skinned mesh of the garment: all share the
        // skeleton.
        let Some((skin, mesh_transform)) = drawn
            .0
            .iter()
            .flatten()
            .find_map(|m| skinned.get(*m).ok().map(|(_, s, t)| (s, t)))
        else {
            continue;
        };
        let Some(ibm) = binds.get(&skin.inverse_bindposes) else {
            continue;
        };
        let matrix = mesh_transform.to_matrix();
        let back = matrix.inverse();
        let bones: Vec<Mat4> = skin
            .joints
            .iter()
            .zip(ibm.iter())
            .map(|(j, ibp)| {
                let world = transforms.get(*j).map_or(Mat4::IDENTITY, |t| t.to_matrix());
                back * world * *ibp
            })
            .collect();
        if bones.len() != model.skeleton.len() {
            continue;
        }
        simulate(&mut cloth, &model, &bones, matrix, dt);
        let Some(state) = cloth.state.as_ref() else {
            continue;
        };
        // The first time: give each simulated mesh its own copy, drawn
        // unskinned where the solver puts it.
        let mut own = cloth_meshes.map(|c| c.clone()).unwrap_or_default();
        if own.0.is_empty() {
            for &i in cloth.meshes.keys() {
                let Some(mesh_entity) = drawn.0.get(i).copied().flatten() else {
                    continue;
                };
                let Ok((mesh, _, _)) = skinned.get(mesh_entity) else {
                    continue;
                };
                let Some(copy) = meshes.get(&mesh.0).cloned() else {
                    continue;
                };
                let handle = meshes.add(copy);
                commands
                    .entity(mesh_entity)
                    .insert((Mesh3d(handle.clone()), NoFrustumCulling))
                    .remove::<SkinnedMesh>();
                own.0.push((i, mesh_entity, handle));
            }
            commands.entity(e).insert(own.clone());
        }
        for (i, _, handle) in &own.0 {
            let Some(pm) = state.posed.get(*i) else {
                continue;
            };
            let Some(mut mesh) = meshes.get_mut(handle) else {
                continue;
            };
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                pm.positions
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>(),
            );
            if !pm.normals.is_empty() {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_NORMAL,
                    pm.normals.iter().map(|n| n.to_array()).collect::<Vec<_>>(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_index_from_label() {
        assert_eq!(mesh_index("Mesh7/Primitive0"), Some(7));
        assert_eq!(mesh_index("Scene0"), None);
    }
}
