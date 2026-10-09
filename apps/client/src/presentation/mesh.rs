//! A skinned model's geometry as the cloth, footwear and bone code read it,
//! with no GPU: what raylib's Mesh and ModelSkeleton gave the Go code. The
//! viewer fills it from Bevy's Mesh assets and the glTF skin.

use bevy::math::Vec3;

use super::bones::Skeleton;

/// SkinnedMeshData is one mesh's bind-pose vertices with their bone
/// weights. Four bones a vertex, as glTF and raylib have it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinnedMeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// Triangle corners, three a triangle; empty for an unindexed mesh,
    /// whose corners are its vertices in order.
    pub indices: Vec<u32>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

impl SkinnedMeshData {
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn skinned(&self) -> bool {
        !self.joints.is_empty() && !self.weights.is_empty() && !self.positions.is_empty()
    }

    /// triangles is the mesh's triangles as vertex indices, three a
    /// triangle.
    pub fn triangles(&self) -> Vec<i32> {
        if self.indices.is_empty() {
            return (0..self.positions.len() as i32).collect();
        }
        self.indices.iter().map(|&i| i as i32).collect()
    }

    /// best_bone is the bone that moves vertex v most.
    pub fn best_bone(&self, v: usize) -> usize {
        let (ids, w) = (&self.joints[v], &self.weights[v]);
        let mut best = 0;
        for j in 1..4 {
            if w[j] > w[best] {
                best = j;
            }
        }
        ids[best] as usize
    }
}

/// ModelMeshes is a whole model: its meshes in file order and the skeleton
/// they're rigged to.
#[derive(Clone, Debug, Default)]
pub struct ModelMeshes {
    pub meshes: Vec<SkinnedMeshData>,
    pub skeleton: Skeleton,
}

impl ModelMeshes {
    /// by_bone groups every mesh's vertices by the bone that moves each
    /// most.
    pub fn by_bone(&self) -> Vec<Vec<Vec3>> {
        let mut out = vec![vec![]; self.skeleton.len()];
        for m in &self.meshes {
            if !m.skinned() {
                continue;
            }
            for (v, p) in m.positions.iter().enumerate() {
                let bone = m.best_bone(v);
                if bone < out.len() {
                    out[bone].push(*p);
                }
            }
        }
        out
    }
}

/// ModelStore is a resource: each loaded model's geometry by path, for the
/// systems that fit cloth and shoes (the viewer fills it from the GLBs; a
/// test inserts its own).
#[derive(bevy::prelude::Resource, Default)]
pub struct ModelStore {
    pub by_path: std::collections::HashMap<String, std::sync::Arc<ModelMeshes>>,
}

impl ModelStore {
    pub fn get(&self, path: &str) -> Option<std::sync::Arc<ModelMeshes>> {
        self.by_path.get(path).cloned()
    }

    pub fn insert(&mut self, path: &str, model: ModelMeshes) {
        self.by_path
            .insert(path.to_string(), std::sync::Arc::new(model));
    }
}
