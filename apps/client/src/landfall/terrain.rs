//! The terrain as it's drawn and walked on, round the height model in
//! `earth_two_world::terrain` (`game/terrain.go` after its height
//! functions). It's drawn in two layers. Round the player, detail chunks
//! ([`CHUNK_SIZE`] across, a vertex every `CHUNK_SIZE / CHUNK_CELLS`)
//! follow them about; they collide, near the player. Everywhere, tiles
//! ([`TILE_SIZE`] across, coarser) show the land to the horizon, sunk out
//! of sight where the chunks are. Each is painted: the soil, the roads, the
//! rock.
//!
//! This module decides what's where: which chunk each slot shows, which
//! colliders stand where, which tiles sink. The meshes and paint are the
//! viewer's (`render.rs`), told by the [`ChunkRebuild`] and [`TileResink`]
//! markers.

use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::{
    kit::{Kit, Placement, quarter_turns},
    terrain::{
        CHUNK_CELLS, CHUNK_SIZE, COLLIDER_RADIUS, GROUND_LEVEL, Sample, TILE_SINK, TILE_SIZE,
        WORLD_SIZE, chunk_centre, chunk_of, grid, ground_height, walk_height,
    },
};

use super::{
    Budget, StreamCentre,
    ground::{Rectangle, footprint},
};

/// The terrain's entities: a detail chunk (its slot), a collider (its
/// slot), a tile.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainChunk {
    pub slot: usize,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainBody {
    pub slot: usize,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TerrainTile;

/// A chunk whose slot has been handed another chunk: its mesh and paint
/// are to be rebuilt where its transform now is (the viewer takes it off).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ChunkRebuild;

/// A tile the detail window has moved over or off: its heights are to be
/// written again with the window's sink (the viewer takes it off).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TileResink;

/// `detailWindow`: the square the detail covers round chunk (ci, cj), on
/// the XZ plane.
pub fn detail_window(ci: i32, cj: i32, radius: i32) -> Rectangle {
    let w = (2 * radius + 1) as f32 * CHUNK_SIZE;
    Rectangle::new(
        (ci - radius) as f32 * CHUNK_SIZE,
        (cj - radius) as f32 * CHUNK_SIZE,
        w,
        w,
    )
}

/// `sinkUnder`: sinks what's inside `win` by [`TILE_SINK`].
pub fn sink_under(win: Rectangle) -> impl Fn(f32, f32) -> f32 + Copy {
    move |x, z| {
        if x > win.x && x < win.x + win.width && z > win.y && z < win.y + win.height {
            TILE_SINK
        } else {
            0.0
        }
    }
}

pub fn no_sink(_x: f32, _z: f32) -> f32 {
    0.0
}

/// `chunkCollider`'s shape: what's walked on in the chunk at (ci, cj), the
/// ground as `walk_height` has it, in the chunk's own frame (its entity
/// sits at its middle).
pub fn chunk_collider_mesh(ci: i32, cj: i32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let c = chunk_centre(ci, cj);
    let (mut v, idx) = grid(c.x, c.y, CHUNK_SIZE, CHUNK_CELLS, walk_height);
    for p in &mut v {
        p.x -= c.x;
        p.z -= c.y;
    }
    let triangles = idx
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    (v, triangles)
}

/// `chunkCollider`: the chunk's ground as an Avian trimesh.
pub fn chunk_collider(ci: i32, cj: i32) -> Collider {
    let (vertices, triangles) = chunk_collider_mesh(ci, cj);
    Collider::trimesh(vertices, triangles)
}

/// The grid point nearest a local vertex coordinate, as `shape` and
/// `writeTileHeights` find it: raylib's plane repeats vertices, so each is
/// snapped back to its grid point.
pub fn grid_index(local: f32, size: f32, step: f32, cells: usize) -> usize {
    let i = (((local + size / 2.0) / step) as f64).round() as i64;
    i.clamp(0, cells as i64) as usize
}

/// `writeTileHeights`: changes only the detail window's masking of a tile.
/// Its unsunk heights are static (one per grid point, from `tile_heights`);
/// no noise or normal rebuild is needed when the player moves. `vertices`
/// are x, y, z triples in the tile's own frame.
pub fn write_tile_heights(
    vertices: &mut [f32],
    cx: f32,
    cz: f32,
    size: f32,
    cells: usize,
    heights: &[f32],
    sink: impl Fn(f32, f32) -> f32,
) {
    let step = size / cells as f32;
    for v in vertices.as_chunks_mut::<3>().0 {
        let (x, z) = (v[0], v[2]);
        let gx = grid_index(x, size, step, cells);
        let gz = grid_index(z, size, step, cells);
        v[1] = GROUND_LEVEL + heights[gz * (cells + 1) + gx] - sink(cx + x, cz + z);
    }
}

/// The heights alone of a grid's samples: a tile's cache.
pub fn tile_heights(samples: &[Sample]) -> Vec<f32> {
    samples.iter().map(|s| s.height).collect()
}

/// Where the tiles are: their middles, row by row, as `spawnTerrain` lays
/// them out across the world.
pub fn tile_centres() -> Vec<Vec2> {
    let h = WORLD_SIZE / 2.0;
    let mut out = Vec::new();
    let mut tz = -h + TILE_SIZE / 2.0;
    while tz < h {
        let mut tx = -h + TILE_SIZE / 2.0;
        while tx < h {
            out.push(Vec2::new(tx, tz));
            tx += TILE_SIZE;
        }
        tz += TILE_SIZE;
    }
    out
}

/// A detail slot: the chunk it's for, and the chunk it shows, until it's
/// rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSlot {
    pub ci: i32,
    pub cj: i32,
    pub bi: i32,
    pub bj: i32,
    pub entity: Entity,
}

/// A collider slot: the chunk it's on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodySlot {
    pub ci: i32,
    pub cj: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileSlot {
    pub cx: f32,
    pub cz: f32,
    pub entity: Entity,
}

/// `terrain`, a resource: where the detail is, and what each of its slots
/// holds.
#[derive(Resource, Debug, Clone)]
pub struct Terrain {
    /// The chunk the detail is round.
    pub ci: i32,
    pub cj: i32,
    /// Chunks each way round the player the detail covers (`detailRadius`).
    pub detail_radius: i32,
    pub chunks_per_frame: usize,
    pub chunks: Vec<ChunkSlot>,
    pub bodies: Vec<BodySlot>,
    pub body_entities: Vec<Entity>,
    pub tiles: Vec<TileSlot>,
    /// Chunk slots waiting to be rebuilt, nearest the player first.
    pub pending: Vec<usize>,
}

impl Terrain {
    /// `bodyAt`: the chunk collider slot `s` should be on, round the
    /// detail's middle.
    pub fn body_at(&self, s: usize) -> BodySlot {
        let side = (2 * COLLIDER_RADIUS + 1) as usize;
        BodySlot {
            ci: self.ci - COLLIDER_RADIUS + (s % side) as i32,
            cj: self.cj - COLLIDER_RADIUS + (s / side) as i32,
        }
    }

    /// The square the detail covers now.
    pub fn window(&self) -> Rectangle {
        detail_window(self.ci, self.cj, self.detail_radius)
    }

    /// `retarget`: hands the chunks the window has left to the ones it's
    /// taken on, and queues every slot that doesn't show its chunk yet
    /// (some may be left over from an earlier move) to be rebuilt, nearest
    /// the player first.
    pub fn retarget(&mut self) {
        let r = self.detail_radius;
        let mut wanted: Vec<(i32, i32)> = Vec::new();
        for dj in -r..=r {
            for di in -r..=r {
                wanted.push((self.ci + di, self.cj + dj));
            }
        }
        let mut free = Vec::new();
        for (s, c) in self.chunks.iter().enumerate() {
            if let Some(i) = wanted.iter().position(|&w| w == (c.ci, c.cj)) {
                wanted.remove(i);
            } else {
                free.push(s);
            }
        }
        for (slot, (ci, cj)) in free.into_iter().zip(wanted) {
            self.chunks[slot].ci = ci;
            self.chunks[slot].cj = cj;
        }
        self.pending.clear();
        for (s, c) in self.chunks.iter().enumerate() {
            if c.ci != c.bi || c.cj != c.bj {
                self.pending.push(s);
            }
        }
        let away = |c: &ChunkSlot, ci: i32, cj: i32| (c.ci - ci).abs().max((c.cj - cj).abs());
        let (ci, cj) = (self.ci, self.cj);
        // Insertion sort, as the Go code: stable, nearest first.
        for i in 1..self.pending.len() {
            let mut j = i;
            while j > 0
                && away(&self.chunks[self.pending[j]], ci, cj)
                    < away(&self.chunks[self.pending[j - 1]], ci, cj)
            {
                self.pending.swap(j, j - 1);
                j -= 1;
            }
        }
    }

    /// `resink`'s choice: the tiles under where the detail is now, and
    /// where it was (`old`), whose heights are to be written again.
    pub fn resink(&self, old: Rectangle) -> Vec<Entity> {
        let win = self.window();
        let overlaps = |r: &Rectangle, cx: f32, cz: f32| {
            r.x < cx + TILE_SIZE / 2.0
                && r.x + r.width > cx - TILE_SIZE / 2.0
                && r.y < cz + TILE_SIZE / 2.0
                && r.y + r.height > cz - TILE_SIZE / 2.0
        };
        self.tiles
            .iter()
            .filter(|t| overlaps(&win, t.cx, t.cz) || overlaps(&old, t.cx, t.cz))
            .map(|t| t.entity)
            .collect()
    }
}

/// `spawnTerrain`: adds the terrain round `at`: the tiles everywhere, the
/// detail and its colliders round `at`. The ground takes shadows but casts
/// none: its gentle slopes would only shade themselves, speckled (the
/// viewer marks it so).
pub fn spawn_terrain(commands: &mut Commands, budget: &Budget, at: Vec3) -> Terrain {
    let (ci, cj) = chunk_of(at.x, at.z);
    let mut t = Terrain {
        ci,
        cj,
        detail_radius: budget.detail_radius,
        chunks_per_frame: budget.chunks_per_frame,
        chunks: Vec::new(),
        bodies: Vec::new(),
        body_entities: Vec::new(),
        tiles: Vec::new(),
        pending: Vec::new(),
    };
    for c in tile_centres() {
        let entity = commands
            .spawn((
                Name::new("terrain tile"),
                TerrainTile,
                Transform::from_xyz(c.x, 0.0, c.y),
            ))
            .id();
        t.tiles.push(TileSlot {
            cx: c.x,
            cz: c.y,
            entity,
        });
    }
    let r = budget.detail_radius;
    let mut slot = 0;
    for dj in -r..=r {
        for di in -r..=r {
            let (ci, cj) = (t.ci + di, t.cj + dj);
            let c = chunk_centre(ci, cj);
            let entity = commands
                .spawn((
                    Name::new("terrain chunk"),
                    TerrainChunk { slot },
                    Transform::from_xyz(c.x, 0.0, c.y),
                ))
                .id();
            t.chunks.push(ChunkSlot {
                ci,
                cj,
                bi: ci,
                bj: cj,
                entity,
            });
            slot += 1;
        }
    }
    let side = (2 * COLLIDER_RADIUS + 1) as usize;
    for s in 0..side * side {
        let b = t.body_at(s);
        let c = chunk_centre(b.ci, b.cj);
        let entity = commands
            .spawn((
                Name::new("terrain body"),
                TerrainBody { slot: s },
                Transform::from_xyz(c.x, 0.0, c.y),
                RigidBody::Static,
                chunk_collider(b.ci, b.cj),
            ))
            .id();
        t.bodies.push(b);
        t.body_entities.push(entity);
    }
    t
}

/// `streamTerrain`: keeps the detail and the colliders round the player as
/// they move: when they cross into another chunk, the chunks that fall out
/// of the window are reused for the ones coming into it (a few a frame,
/// nearest first), the colliders round them likewise (at once: they're
/// what's stood on), and the tiles sink where the detail now is.
pub fn stream_terrain(
    mut commands: Commands,
    terrain: Option<ResMut<Terrain>>,
    centre: Query<Entity, With<StreamCentre>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(mut t) = terrain else {
        return;
    };
    let Ok(player) = centre.single() else {
        return;
    };
    let Ok(at) = transforms.get(player).map(|tr| tr.translation) else {
        return;
    };
    let (ci, cj) = chunk_of(at.x, at.z);
    if ci != t.ci || cj != t.cj {
        let old = t.window();
        t.ci = ci;
        t.cj = cj;
        t.retarget();
        for tile in t.resink(old) {
            commands.entity(tile).insert(TileResink);
        }
        for s in 0..t.bodies.len() {
            let want = t.body_at(s);
            if t.bodies[s] == want {
                continue;
            }
            t.bodies[s] = want;
            let entity = t.body_entities[s];
            let c = chunk_centre(want.ci, want.cj);
            if let Ok(mut tr) = transforms.get_mut(entity) {
                tr.translation = Vec3::new(c.x, 0.0, c.y);
            }
            commands
                .entity(entity)
                .insert(chunk_collider(want.ci, want.cj));
        }
    }
    if t.pending.is_empty() {
        return;
    }
    let n = t.chunks_per_frame.min(t.pending.len());
    let rebuild: Vec<usize> = t.pending.drain(..n).collect();
    for s in rebuild {
        let slot = &mut t.chunks[s];
        slot.bi = slot.ci;
        slot.bj = slot.cj;
        let c = chunk_centre(slot.ci, slot.cj);
        if let Ok(mut tr) = transforms.get_mut(slot.entity) {
            tr.translation = Vec3::new(c.x, 0.0, c.y);
        }
        commands.entity(slot.entity).insert(ChunkRebuild);
    }
}

/// `standOn`: how high the ground is under a placed piece: its lowest,
/// under the middle and the corners of what it collides with.
pub fn stand_on(kit: &Kit, p: &Placement) -> f32 {
    let at = p.at();
    let mut low = ground_height(at.x, at.z);
    let Some(piece) = kit.pieces.get(&p.piece) else {
        return low;
    };
    let turn = quarter_turns(p.turns);
    for c in &piece.colliders {
        let (r, _) = footprint(c, at, turn);
        for x in [r.x, r.x + r.width] {
            for z in [r.y, r.y + r.height] {
                low = low.min(ground_height(x, z));
            }
        }
    }
    low
}

/// `terrainAround`: spawns the ground's colliders round (x, z), `radius`
/// chunks each way, standing still: for a world without the streaming
/// (tests).
pub fn terrain_around(commands: &mut Commands, x: f32, z: f32, radius: i32) -> Vec<Entity> {
    let (ci, cj) = chunk_of(x, z);
    let mut out = Vec::new();
    for dj in -radius..=radius {
        for di in -radius..=radius {
            let c = chunk_centre(ci + di, cj + dj);
            out.push(
                commands
                    .spawn((
                        Name::new("terrain around"),
                        Transform::from_xyz(c.x, 0.0, c.y),
                        RigidBody::Static,
                        chunk_collider(ci + di, cj + dj),
                    ))
                    .id(),
            );
        }
    }
    out
}

/// `terrainTileVertices` of the Go tests: a tile's vertices as raylib's
/// plane repeats them, one triple per triangle corner, at height 0.
pub fn tile_triangle_vertices(cells: usize) -> Vec<f32> {
    let (points, indices) = grid(0.0, 0.0, TILE_SIZE, cells, |_, _| 0.0);
    let mut vertices = Vec::with_capacity(indices.len() * 3);
    for i in indices {
        let p = points[i as usize];
        vertices.extend_from_slice(&[p.x, p.y, p.z]);
    }
    vertices
}
