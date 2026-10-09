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
use super::outfit::{Garment, ModelParts, ModelPath, Slot, load_wardrobe};
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

/// CPU cloth stops GPU skinning, but still needs the live joints every frame.
#[derive(Component, Clone)]
struct ClothSkin(SkinnedMesh);

/// The body scene this garment's joint palette is bound to.
#[derive(Component)]
struct GarmentSkeleton(Entity);

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
                (
                    index_models,
                    spawn_scenes,
                    wire_scenes,
                    bind_garments,
                    apply_parts,
                )
                    .chain()
                    .in_set(super::PresentationSystems::Models),
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
            .remove::<(Animated, MeshEntities, ClothMeshes, GarmentSkeleton)>();
    }
}

/// wire_scenes finds, once a scene is in, the entity Bevy animates and the
/// entities drawing each mesh, so the clip clock and the model parts can
/// reach them.
fn wire_scenes(
    mut commands: Commands,
    assets: Res<AssetServer>,
    scenes: Query<(Entity, &ModelScene), Without<MeshEntities>>,
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
        if meshes.iter().any(Option::is_none) {
            continue; // Wait for the complete scene, including animation-free garments.
        }
        commands.entity(e).insert(MeshEntities(meshes));
        if let Some(animated) = animated {
            commands.entity(e).insert(Animated(animated));
        }
    }
}

/// Garment GLBs contain bind-pose skeletons, not clips. Share the body's
/// animated joint entities by name, preserving each garment's palette order
/// and inverse bind matrices. Separate characters keep separate skeletons.
#[allow(clippy::type_complexity)]
fn bind_garments(
    mut commands: Commands,
    garments: Query<(Entity, &ChildOf, &MeshEntities, Option<&GarmentSkeleton>), With<Garment>>,
    bodies: Query<(&ModelScene, &MeshEntities), Without<Garment>>,
    names: Query<&Name>,
    children: Query<&Children>,
    mut skins: Query<&mut SkinnedMesh>,
) {
    for (entity, parent, meshes, attached) in &garments {
        let Ok((body_scene, body_meshes)) = bodies.get(parent.parent()) else {
            continue;
        };
        if attached.is_some_and(|a| a.0 == body_scene.root) {
            continue;
        }
        let Some(body_skin) = body_meshes
            .0
            .iter()
            .flatten()
            .find_map(|e| skins.get(*e).ok())
        else {
            continue;
        };
        let joints: HashMap<String, Entity> = body_skin
            .joints
            .iter()
            .filter_map(|e| {
                names
                    .get(*e)
                    .ok()
                    .map(|name| (name.as_str().to_owned(), *e))
            })
            .collect();
        // Resolve the complete palette before changing any mesh; delayed
        // scene loading must never leave a garment partially attached.
        let remapped: Option<Vec<_>> = meshes
            .0
            .iter()
            .flatten()
            .map(|e| {
                let skin = skins.get(*e).ok()?;
                let mapped: Option<Vec<Entity>> = skin
                    .joints
                    .iter()
                    .map(|j| joints.get(names.get(*j).ok()?.as_str()).copied())
                    .collect();
                Some((*e, mapped?))
            })
            .collect();
        let Some(remapped) = remapped else { continue };
        for (mesh, joints) in remapped {
            skins.get_mut(mesh).unwrap().joints = joints.clone();
            // Material adoption can precede the body's asynchronous scene
            // readiness. Rebind any outline already cloned from this mesh.
            for child in children.iter_descendants(mesh) {
                if let Ok(mut skin) = skins.get_mut(child) {
                    skin.joints = joints.clone();
                }
            }
        }
        commands
            .entity(entity)
            .insert(GarmentSkeleton(body_scene.root));
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
    drawn: Query<(
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&crate::shading::SourceMaterial>,
    )>,
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
                && let Ok((material, source)) = drawn.get(mesh)
                && let Some(handle) = material.map(|m| &m.0).or_else(|| source.map(|m| &m.0))
                && let Some(own) = materials.get(handle).cloned()
            {
                let handle = toned.entry((e, i)).or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color_texture: Some(assets.load(tone.clone())),
                        ..own
                    })
                });
                entity
                    .remove::<MeshMaterial3d<crate::shading::PaintedMaterial>>()
                    .insert(MeshMaterial3d(handle.clone()));
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
    mut garments: Query<
        (
            Entity,
            &mut Cloth,
            &ModelScene,
            &MeshEntities,
            Option<&mut ClothMeshes>,
        ),
        With<GarmentSkeleton>,
    >,
    children: Query<&Children>,
    outlines: Query<(), With<MeshMaterial3d<crate::shading::OutlineMaterial>>>,
    skinned: Query<(
        &Mesh3d,
        Option<&SkinnedMesh>,
        Option<&ClothSkin>,
        &GlobalTransform,
    )>,
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
        let Some((skin, mesh_transform)) = drawn.0.iter().flatten().find_map(|m| {
            let (_, gpu, cpu, tr) = skinned.get(*m).ok()?;
            Some((gpu.or_else(|| cpu.map(|s| &s.0))?, tr))
        }) else {
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
                let Ok((mesh, Some(skin), _, _)) = skinned.get(mesh_entity) else {
                    continue;
                };
                let Some(mut copy) = meshes.get(&mesh.0).cloned() else {
                    continue;
                };
                // The shader specializes from vertex attributes, not just
                // SkinnedMesh: CPU-posed vertices must use an unskinned layout.
                copy.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
                copy.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
                let handle = meshes.add(copy);
                for child in children.iter_descendants(mesh_entity) {
                    if outlines.contains(child) {
                        commands
                            .entity(child)
                            .insert(Mesh3d(handle.clone()))
                            .remove::<SkinnedMesh>();
                    }
                }
                commands
                    .entity(mesh_entity)
                    .insert((
                        Mesh3d(handle.clone()),
                        NoFrustumCulling,
                        ClothSkin(skin.clone()),
                    ))
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
    #[test]
    fn garments_share_only_their_own_bodys_joints_in_palette_order() {
        let mut app = App::new();
        app.add_systems(Update, bind_garments);
        let mut expected = Vec::new();
        for _ in 0..2 {
            let head = app.world_mut().spawn(Name::new("head")).id();
            let hand = app.world_mut().spawn(Name::new("hand")).id();
            let body_mesh = app
                .world_mut()
                .spawn(SkinnedMesh {
                    inverse_bindposes: default(),
                    joints: vec![head, hand],
                })
                .id();
            let body = app
                .world_mut()
                .spawn((
                    ModelScene {
                        path: "body".into(),
                        root: body_mesh,
                    },
                    MeshEntities(vec![Some(body_mesh)]),
                ))
                .id();
            let old_head = app.world_mut().spawn(Name::new("head")).id();
            let old_hand = app.world_mut().spawn(Name::new("hand")).id();
            let mesh = app
                .world_mut()
                .spawn(SkinnedMesh {
                    inverse_bindposes: default(),
                    joints: vec![old_hand, old_head],
                })
                .id();
            app.world_mut().spawn((
                Garment {
                    slot: Slot::Hair,
                    skin: vec![],
                    footwear: None,
                },
                ChildOf(body),
                MeshEntities(vec![Some(mesh)]),
            ));
            let outline = app
                .world_mut()
                .spawn((
                    ChildOf(mesh),
                    SkinnedMesh {
                        inverse_bindposes: default(),
                        joints: vec![old_hand, old_head],
                    },
                ))
                .id();
            expected.push((mesh, vec![hand, head]));
            expected.push((outline, vec![hand, head]));
        }
        app.update();
        for (mesh, expected) in expected {
            assert_eq!(
                app.world().get::<SkinnedMesh>(mesh).unwrap().joints,
                expected
            );
        }
    }

    #[test]
    fn cpu_cloth_keeps_following_the_skeleton_after_gpu_skinning_is_removed() {
        use super::super::verlet::ClothMeshSpec;
        use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .init_resource::<ModelStore>()
            .insert_resource(Controls { enabled: true })
            .add_systems(Update, simulate_cloth);
        let vertices = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        app.world_mut().resource_mut::<ModelStore>().insert(
            "cloth",
            ModelMeshes {
                meshes: vec![SkinnedMeshData {
                    positions: vertices.clone(),
                    normals: vec![Vec3::Z; 3],
                    joints: vec![[0; 4]; 3],
                    weights: vec![[1., 0., 0., 0.]; 3],
                    ..default()
                }],
                skeleton: Skeleton {
                    names: vec!["head".into()],
                    parents: vec![-1],
                    bind: vec![Transform::IDENTITY],
                },
            },
        );
        let joint = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
        let binds = app
            .world_mut()
            .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
            .add(SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY]));
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD,
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vertices.iter().map(|v| v.to_array()).collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(vec![[0; 4]; 3]),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, vec![[1., 0., 0., 0.]; 3]),
        );
        let mesh_entity = app
            .world_mut()
            .spawn((
                Mesh3d(mesh.clone()),
                SkinnedMesh {
                    inverse_bindposes: binds,
                    joints: vec![joint],
                },
                GlobalTransform::IDENTITY,
            ))
            .id();
        let outline_skin = app.world().get::<SkinnedMesh>(mesh_entity).unwrap().clone();
        let outline = app
            .world_mut()
            .spawn((
                ChildOf(mesh_entity),
                Mesh3d(mesh.clone()),
                outline_skin,
                MeshMaterial3d::<crate::shading::OutlineMaterial>(default()),
            ))
            .id();
        app.world_mut().spawn((
            Cloth {
                meshes: [(
                    0,
                    ClothMeshSpec {
                        freedom: vec![0.; 3],
                    },
                )]
                .into(),
                ..default()
            },
            GarmentSkeleton(joint),
            ModelScene {
                path: "cloth".into(),
                root: mesh_entity,
            },
            MeshEntities(vec![Some(mesh_entity)]),
        ));
        app.update();
        assert!(app.world().get::<SkinnedMesh>(mesh_entity).is_none());
        let own = app.world().get::<Mesh3d>(mesh_entity).unwrap().0.clone();
        assert_ne!(own, mesh);
        assert_eq!(app.world().get::<Mesh3d>(outline).unwrap().0, own);
        assert!(app.world().get::<SkinnedMesh>(outline).is_none());
        let assets = app.world().resource::<Assets<Mesh>>();
        assert!(
            assets
                .get(&own)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
                .is_none()
        );
        assert!(
            assets
                .get(&own)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
                .is_none()
        );
        assert!(
            assets
                .get(&mesh)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
                .is_some()
        );
        for x in [0.2, 0.4, 0.6] {
            *app.world_mut().get_mut::<GlobalTransform>(joint).unwrap() =
                GlobalTransform::from_translation(Vec3::X * x);
            app.update();
            let assets = app.world().resource::<Assets<Mesh>>();
            let Some(VertexAttributeValues::Float32x3(positions)) = assets
                .get(&own)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions");
            };
            for (actual, rest) in positions.iter().zip(&vertices) {
                assert!(Vec3::from(*actual).distance(*rest + Vec3::X * x) < 1e-5);
            }
        }
    }
}
