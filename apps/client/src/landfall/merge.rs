//! Merging (`world/merge.go`). The renderer draws each model on its own, a
//! draw for each of its meshes, and a town is thousands of small pieces:
//! deck plates, catwalks, rocks, crates. Drawn one at a time, they cost
//! more in draws than in anything they draw, and twice, as the shadow map
//! draws them again. So the small pieces of a layout are merged by where
//! they stand, in squares [`MERGE_CELL`] across:
//!
//!   - every copy of a piece in a square, into one mesh (each piece is
//!     painted on its own texture, so only copies can share a draw), and
//!     what glows on them into another;
//!   - and the shadow every opaque piece in a square casts, into one mesh
//!     of their shapes alone (the shadow map needs no paint).
//!
//! What a piece collides with, its ladders and its lights are placed as
//! the layout places them. Pieces of more than [`MERGE_VERTICES`]
//! vertices, or reaching further than [`MERGE_RADIUS`] from their middle,
//! are drawn on their own, as are vehicles: they're few, and the big ones
//! are seen from far off.
//!
//! The plan here is rendering-free: it takes each piece's meshes as plain
//! vertex data and gives back the merged meshes in a fixed order, so the
//! world's the same each time. The viewer reads the loaded models into it
//! and uploads what comes out.

use std::collections::BTreeMap;

use bevy::prelude::*;
use earth_two_world::kit::Placement;

pub const MERGE_CELL: f32 = 32.0;
pub const MERGE_VERTICES: usize = 5000;
pub const MERGE_RADIUS: f32 = 8.0;
/// The most vertices one mesh can have: its triangles index them in 16
/// bits.
pub const MESH_MOST: usize = u16::MAX as usize;

/// Marks a mesh merged from pieces. `center` and `radius` bound it, in
/// its own frame. `piece` is the radius of the biggest piece in it: how
/// far off it's seen is how far off they would be. `shadow` is whether
/// it's only the shadow they cast; `casts`, whether it casts its own (if
/// not, its pieces' shadow is a `shadow` mesh's).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Merged {
    pub center: Vec3,
    pub radius: f32,
    pub piece: f32,
    pub shadow: bool,
    pub casts: bool,
}

/// The placements the viewer is to merge once their models load: the
/// layout as it stands on the terrain.
#[derive(Resource, Debug, Clone, Default)]
pub struct MergeRequest {
    pub placements: Vec<Placement>,
}

/// A model's mesh as plain data, in the model's frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshSource {
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub texcoords: Option<Vec<Vec2>>,
    pub indices: Option<Vec<u32>>,
    /// Skinned meshes aren't merged.
    pub skinned: bool,
}

/// A merged mesh's data: `render.MeshData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub texcoords: Vec<Vec2>,
    pub indices: Vec<u16>,
}

/// `look`: how a merged mesh is painted: the texture and colour its pieces
/// are, and what glows of them. `texture` is the renderer's id for it (0
/// for none).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Look {
    pub texture: u64,
    pub emissive: [u8; 4],
    pub color: [u8; 4],
}

/// A square and a look: what's merged together. Ordered as the Go code
/// spawns them: by square, then texture, then glow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MergeKey {
    pub cx: i32,
    pub cz: i32,
    pub look: Look,
}

/// `merging`: a merged mesh being built: its meshes so far (each new one
/// once the last is full), the biggest piece in it, and whether it casts
/// its own shadow.
#[derive(Debug, Clone, Default)]
pub struct Merging {
    pub meshes: Vec<MeshData>,
    pub piece: f32,
    pub casts: bool,
}

impl Merging {
    /// `add`: appends a mesh's vertices and triangles, moved by `place`, to
    /// the last of the meshes (or a new one if it's full). A shape (for the
    /// shadow) has no normals or texture coordinates.
    pub fn add(&mut self, src: &MeshSource, place: impl Fn(Vec3) -> Vec3, turn: Quat, shape: bool) {
        let n = src.positions.len();
        if n == 0 || n > MESH_MOST {
            return;
        }
        if self
            .meshes
            .last()
            .is_none_or(|d| d.positions.len() + n > MESH_MOST)
        {
            self.meshes.push(MeshData::default());
        }
        let d = self.meshes.last_mut().expect("just pushed");
        let first = d.positions.len();
        d.positions.extend(src.positions.iter().map(|&v| place(v)));
        if !shape {
            match &src.normals {
                Some(normals) => d.normals.extend(normals.iter().map(|&v| turn * v)),
                None => d.normals.extend(std::iter::repeat_n(Vec3::ZERO, n)),
            }
            match &src.texcoords {
                Some(uv) => d.texcoords.extend_from_slice(uv),
                None => d.texcoords.extend(std::iter::repeat_n(Vec2::ZERO, n)),
            }
        }
        match &src.indices {
            Some(indices) => d
                .indices
                .extend(indices.iter().map(|&i| (first as u32 + i) as u16)),
            None => d.indices.extend((0..n).map(|i| (first + i) as u16)),
        }
    }
}

/// `mergeable`: whether a model is small enough to merge, and its radius
/// about its middle.
pub fn mergeable(meshes: &[MeshSource]) -> (f32, bool) {
    let mut total = 0;
    let mut lo = Vec3::MAX;
    let mut hi = Vec3::MIN;
    for mesh in meshes {
        let n = mesh.positions.len();
        total += n;
        if mesh.positions.is_empty() || mesh.skinned || total > MERGE_VERTICES {
            return (0.0, false);
        }
        for v in &mesh.positions {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    let r = lo.distance(hi) / 2.0;
    (r, total > 0 && r <= MERGE_RADIUS)
}

/// The square a piece at `at` is merged in.
pub fn cell_of(at: Vec3) -> (i32, i32) {
    (
        ((at.x / MERGE_CELL) as f64).floor() as i32,
        ((at.z / MERGE_CELL) as f64).floor() as i32,
    )
}

/// `cellOrigin`: the middle of a square pieces are merged by, on the ground.
pub fn cell_origin(cell: (i32, i32)) -> Vec3 {
    Vec3::new(
        (cell.0 as f32 + 0.5) * MERGE_CELL,
        0.0,
        (cell.1 as f32 + 0.5) * MERGE_CELL,
    )
}

/// `merged`: the [`Merged`] of a mesh of `d`.
pub fn merged(d: &MeshData, piece: f32, shadow: bool, casts: bool) -> Merged {
    let mut lo = d.positions[0];
    let mut hi = d.positions[0];
    for v in &d.positions {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    Merged {
        center: (lo + hi) * 0.5,
        radius: lo.distance(hi) / 2.0,
        piece,
        shadow,
        casts,
    }
}

/// A mesh of a piece as the plan takes it: its data, its look, and whether
/// its texture is opaque (has no alpha to cut a shadow out by).
#[derive(Debug, Clone)]
pub struct PieceMesh {
    pub source: MeshSource,
    pub look: Look,
    pub opaque: bool,
}

/// One merged mesh to upload: in which square, painted how (none for a
/// shadow), its data, and its [`Merged`].
#[derive(Debug, Clone)]
pub struct MergedMesh {
    pub cell: (i32, i32),
    pub look: Option<Look>,
    pub data: MeshData,
    pub merged: Merged,
}

/// `PlaceMerged`'s merging, as data: the pieces that merge are added, and
/// the merged meshes come out in a fixed order.
#[derive(Debug, Clone, Default)]
pub struct MergePlan {
    groups: BTreeMap<MergeKey, Merging>,
    shadows: BTreeMap<(i32, i32), Merging>,
}

impl MergePlan {
    /// Adds a mergeable piece at `at` turned by `turn`, of `radius`
    /// (`mergeable`'s), with its meshes.
    pub fn add_piece(&mut self, at: Vec3, turn: Quat, radius: f32, meshes: &[PieceMesh]) {
        let cell = cell_of(at);
        let origin = cell_origin(cell);
        let place = |v: Vec3| at - origin + turn * v;
        for mesh in meshes {
            let key = MergeKey {
                cx: cell.0,
                cz: cell.1,
                look: mesh.look,
            };
            let g = self.groups.entry(key).or_insert_with(|| Merging {
                casts: !mesh.opaque,
                ..Default::default()
            });
            g.piece = g.piece.max(radius);
            g.add(&mesh.source, place, turn, false);
            if mesh.opaque {
                let sh = self.shadows.entry(cell).or_default();
                sh.piece = sh.piece.max(radius);
                sh.add(&mesh.source, place, turn, true);
            }
        }
    }

    /// The merged meshes, in the order they're spawned: the painted ones by
    /// square, texture and glow, then the shadows by square.
    pub fn finish(self) -> Vec<MergedMesh> {
        let mut out = Vec::new();
        for (key, g) in self.groups {
            for d in g.meshes {
                let m = merged(&d, g.piece, false, g.casts);
                out.push(MergedMesh {
                    cell: (key.cx, key.cz),
                    look: Some(key.look),
                    data: d,
                    merged: m,
                });
            }
        }
        for (cell, sh) in self.shadows {
            for d in sh.meshes {
                let m = merged(&d, sh.piece, true, true);
                out.push(MergedMesh {
                    cell,
                    look: None,
                    data: d,
                    merged: m,
                });
            }
        }
        out
    }
}
