//! The Fringe's scatter (`game/scatter.go`): dry brush and stones, and now
//! and then a boulder or a dead tree, wherever the player goes, so the open
//! ground between the seats isn't bare. It's laid out cell by cell from a
//! fixed seed, so a place always has the same things on it, and it follows
//! the player: a window of cells round them, its slots handed on to the
//! cells coming into it as they move, as the terrain's chunks are. Nothing
//! is spawned or despawned once it's made; what isn't needed is parked out
//! of sight under the world.

use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::{
    kit::Kit,
    terrain::{GROUND_LEVEL, PADS_AT, fbm, ground_height, smoothstep},
};

use super::{
    StreamCentre,
    ground::{footprint, level_distance},
};
use crate::world::{KitAsset, PieceModel};

/// Metres across a cell.
pub const SCATTER_CELL: f32 = 40.0;
/// The most things a cell holds.
pub const SCATTER_PER: usize = 14;
/// The most colliders a thing has.
pub const SCATTER_BODIES: usize = 2;
/// How far the scatter keeps from level ground: the seats, and the roads
/// between them.
pub const SCATTER_CLEAR: f32 = 8.0;
/// How deep what isn't needed is put.
pub const SCATTER_PARKED: f32 = -1000.0;

/// A piece the scatter uses: how often, out of the weights' sum, and, for
/// those without colliders, how much it's scaled up or down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScatterKind {
    pub piece: &'static str,
    pub weight: f32,
    pub min_s: f32,
    pub max_s: f32,
}

pub const SCATTER_KINDS: [ScatterKind; 5] = [
    ScatterKind {
        piece: "dry_brush",
        weight: 40.0,
        min_s: 1.3,
        max_s: 2.4,
    },
    ScatterKind {
        piece: "rock_small",
        weight: 26.0,
        min_s: 1.0,
        max_s: 1.0,
    },
    ScatterKind {
        piece: "rock_large",
        weight: 10.0,
        min_s: 1.0,
        max_s: 1.0,
    },
    ScatterKind {
        piece: "dead_tree",
        weight: 4.0,
        min_s: 1.0,
        max_s: 1.0,
    },
    ScatterKind {
        piece: "quiver_tree",
        weight: 4.0,
        min_s: 1.0,
        max_s: 1.0,
    },
];

/// How many tufts of brush grow round one, and how far out.
pub const BRUSH_CLUMP: usize = 3;
pub const BRUSH_SPREAD: f32 = 1.6;

/// The ground kept free of anything to run into: round the Pads, where the
/// vehicles are tried out over the dunes (see dunes_test.go). Brush and
/// mounds still grow there.
pub const SCATTER_NO_BODIES: [(Vec2, f32); 1] = [(PADS_AT, 600.0)];

/// One thing in a cell: which piece, where its origin stands, turned `yaw`
/// about up, and scaled.
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterThing {
    pub piece: &'static str,
    pub at: Vec3,
    pub yaw: f32,
    pub scale: f32,
}

/// `scatterIn`: what's in cell (ci, cj): the same every time. Its heights
/// are the ground's at each thing's middle; the stream stands them on it.
pub fn scatter_in(ci: i32, cj: i32) -> Vec<ScatterThing> {
    let mut seed: u32 = (ci as u32).wrapping_mul(0x9e3779b1) ^ (cj as u32).wrapping_mul(0x85ebca77);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed & 0xffffff) as f32 / 0xffffff as f32
    };
    for _ in 0..3 {
        next();
    }
    let (x0, z0) = (ci as f32 * SCATTER_CELL, cj as f32 * SCATTER_CELL);
    // Some stretches are barer than others.
    let busy = fbm(x0 / 300.0 + 17.0, z0 / 300.0 - 5.0);
    let n = 2 + (6.0 * smoothstep(0.3, 0.7, busy)) as i32;
    let total: f32 = SCATTER_KINDS.iter().map(|k| k.weight).sum();
    let mut out: Vec<ScatterThing> = Vec::new();
    let put =
        |out: &mut Vec<ScatterThing>, kind: ScatterKind, x: f32, z: f32, yaw: f32, size: f32| {
            if out.len() >= SCATTER_PER || level_distance(x, z) < SCATTER_CLEAR {
                return;
            }
            if kind.min_s == kind.max_s && no_bodies(x, z) {
                return;
            }
            out.push(ScatterThing {
                piece: kind.piece,
                at: Vec3::new(x, ground_height(x, z), z),
                yaw,
                scale: kind.min_s + (kind.max_s - kind.min_s) * size,
            });
        };
    for _ in 0..n {
        let x = x0 + next() * SCATTER_CELL;
        let z = z0 + next() * SCATTER_CELL;
        let mut pick = next() * total;
        let yaw = next() * 2.0 * std::f32::consts::PI;
        let size = next();
        let mut kind = SCATTER_KINDS[SCATTER_KINDS.len() - 1];
        for k in SCATTER_KINDS {
            if pick < k.weight {
                kind = k;
                break;
            }
            pick -= k.weight;
        }
        put(&mut out, kind, x, z, yaw, size);
        if kind.piece != "dry_brush" {
            continue;
        }
        // Brush grows in clumps.
        for _ in 0..BRUSH_CLUMP {
            let a = next() * 2.0 * std::f32::consts::PI;
            let d = next() * BRUSH_SPREAD;
            let cx = x + d * (a as f64).cos() as f32;
            let cz = z + d * (a as f64).sin() as f32;
            let yaw = next() * 2.0 * std::f32::consts::PI;
            let size = next() * size;
            put(&mut out, kind, cx, cz, yaw, size);
        }
    }
    out
}

/// `noBodies`: whether (x, z) is kept free of colliders.
pub fn no_bodies(x: f32, z: f32) -> bool {
    SCATTER_NO_BODIES
        .iter()
        .any(|(at, radius)| at.distance(Vec2::new(x, z)) < *radius)
}

pub fn scatter_cell_of(x: f32, z: f32) -> (i32, i32) {
    (
        ((x / SCATTER_CELL) as f64).floor() as i32,
        ((z / SCATTER_CELL) as f64).floor() as i32,
    )
}

/// `scatterWindow`: the cells round (ci, cj).
pub fn scatter_window(ci: i32, cj: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for dj in -radius..=radius {
        for di in -radius..=radius {
            out.push((ci + di, cj + dj));
        }
    }
    out
}

/// `scatter`, a resource: the window's middle cell, and the cell each slot
/// holds.
#[derive(Resource, Debug, Default, Clone)]
pub struct Scatter {
    pub ci: i32,
    pub cj: i32,
    pub slots: Vec<(i32, i32)>,
    pub made: bool,
}

/// A slot's things and their colliders.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScatterModel {
    pub slot: usize,
    pub item: usize,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScatterBody {
    pub slot: usize,
    pub item: usize,
    pub n: usize,
}

/// How a slot's thing is placed: its model, its transform, and its
/// colliders by number, each a transform and a box's size.
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterPose {
    pub model: String,
    pub at: Transform,
    pub colliders: Vec<(Transform, [f32; 3])>,
}

/// `scatterPose`: how a slot's item-th thing is placed, in a cell holding
/// `things`: its model, its transform (stood on the ground, at its lowest
/// under it), and its colliders by number, each a transform and a box. An
/// item past the cell's things, and a collider past a thing's, are parked
/// out of sight.
pub fn scatter_pose(
    kit: &Kit,
    things: &[ScatterThing],
    item: usize,
    cell: (i32, i32),
) -> ScatterPose {
    let parked = Vec3::new(
        cell.0 as f32 * SCATTER_CELL,
        SCATTER_PARKED,
        cell.1 as f32 * SCATTER_CELL,
    );
    let park = || (Transform::from_translation(parked), [0.1, 0.1, 0.1]);
    let model_of = |piece: &str| {
        kit.pieces
            .get(piece)
            .map(|p| p.model.clone())
            .unwrap_or_default()
    };
    let Some(t) = things.get(item) else {
        return ScatterPose {
            model: model_of(SCATTER_KINDS[0].piece),
            at: Transform::from_translation(parked),
            colliders: (0..SCATTER_BODIES).map(|_| park()).collect(),
        };
    };
    let colliders = kit
        .pieces
        .get(t.piece)
        .map(|p| p.colliders.clone())
        .unwrap_or_default();
    let turn = Quat::from_axis_angle(Vec3::Y, t.yaw);
    let mut pos = t.at;
    for c in &colliders {
        let (r, _) = footprint(c, pos, turn);
        for x in [r.x, r.x + r.width] {
            for z in [r.y, r.y + r.height] {
                pos.y = pos.y.min(ground_height(x, z));
            }
        }
    }
    // A little into the ground, so no edge of it floats on a slope.
    pos.y += GROUND_LEVEL - 0.08 * t.scale;
    let at = Transform::from_translation(pos)
        .with_rotation(turn)
        .with_scale(Vec3::splat(t.scale));
    let cols = (0..SCATTER_BODIES)
        .map(|n| match colliders.get(n) {
            Some(c) => {
                let (center, rot) = c.in_frame(pos, turn);
                (
                    Transform::from_translation(center).with_rotation(rot),
                    c.size,
                )
            }
            None => park(),
        })
        .collect();
    ScatterPose {
        model: model_of(t.piece),
        at,
        colliders: cols,
    }
}

/// The kit the scatter places from: the one Landfall loaded.
fn loaded_kit(kits: &Assets<KitAsset>) -> Option<&Kit> {
    kits.iter().map(|(_, k)| &k.0).next()
}

/// `streamScatter`: makes the scatter's slots round the player the first
/// time, and after that, when they move into another cell, hands the slots
/// the window's left to the cells it's taken on, and moves their things
/// there.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn stream_scatter(
    mut commands: Commands,
    mut s: ResMut<Scatter>,
    budget: Res<super::Budget>,
    kits: Res<Assets<KitAsset>>,
    centre: Query<&Transform, With<StreamCentre>>,
    mut models: Query<(&mut Transform, &mut PieceModel, &ScatterModel), Without<StreamCentre>>,
    mut bodies: Query<
        (Entity, &mut Transform, &ScatterBody),
        (Without<StreamCentre>, Without<ScatterModel>),
    >,
) {
    let Some(kit) = loaded_kit(&kits) else {
        return;
    };
    let Ok(player) = centre.single() else {
        return;
    };
    let (ci, cj) = scatter_cell_of(player.translation.x, player.translation.z);
    if !s.made {
        s.ci = ci;
        s.cj = cj;
        s.made = true;
        s.slots = scatter_window(ci, cj, budget.scatter_radius);
        for (slot, cell) in s.slots.clone().into_iter().enumerate() {
            let things = scatter_in(cell.0, cell.1);
            for item in 0..SCATTER_PER {
                let pose = scatter_pose(kit, &things, item, cell);
                commands.spawn((
                    Name::new("scatter"),
                    PieceModel(pose.model),
                    pose.at,
                    ScatterModel { slot, item },
                ));
                for (n, (where_, size)) in pose.colliders.into_iter().enumerate() {
                    commands.spawn((
                        Name::new("scatter body"),
                        where_,
                        RigidBody::Static,
                        Collider::cuboid(size[0], size[1], size[2]),
                        ScatterBody { slot, item, n },
                    ));
                }
            }
        }
        return;
    }
    if ci == s.ci && cj == s.cj {
        return;
    }
    s.ci = ci;
    s.cj = cj;
    let want = scatter_window(ci, cj, budget.scatter_radius);
    let mut held = Vec::new();
    let mut free = Vec::new();
    for (slot, c) in s.slots.iter().enumerate() {
        if want.contains(c) {
            held.push(*c);
        } else {
            free.push(slot);
        }
    }
    let mut moved: Vec<(usize, Vec<ScatterThing>)> = Vec::new();
    let mut free = free.into_iter();
    for c in want {
        if held.contains(&c) {
            continue;
        }
        let Some(slot) = free.next() else {
            break;
        };
        s.slots[slot] = c;
        moved.push((slot, scatter_in(c.0, c.1)));
    }
    if moved.is_empty() {
        return;
    }
    for (mut tr, mut model, id) in &mut models {
        let Some((_, things)) = moved.iter().find(|(slot, _)| *slot == id.slot) else {
            continue;
        };
        let pose = scatter_pose(kit, things, id.item, s.slots[id.slot]);
        model.0 = pose.model;
        *tr = pose.at;
    }
    for (e, mut tr, id) in &mut bodies {
        let Some((_, things)) = moved.iter().find(|(slot, _)| *slot == id.slot) else {
            continue;
        };
        let pose = scatter_pose(kit, things, id.item, s.slots[id.slot]);
        let (where_, size) = pose.colliders[id.n];
        *tr = where_;
        commands
            .entity(e)
            .insert(Collider::cuboid(size[0], size[1], size[2]));
    }
}
