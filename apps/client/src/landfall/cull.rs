//! What's drawn (`game/cull.go`). The world's small pieces are thousands,
//! and the renderer draws every one it isn't told to hide, behind the
//! camera or not, and again into the shadow map. So each is hidden when
//! the camera can't see it: behind it, off the sides of the view, or
//! further than [`DRAW_DISTANCE`]. Bigger pieces are drawn further, by
//! [`SIGHT_RANGE`]: the dome, the Second Light and the drifter are
//! landmarks across the Fringe. Those out of view but near enough to throw
//! a shadow into it are drawn only into the shadow map.
//!
//! The decisions are the [`Drawn`] component, set only when it changes;
//! the viewer turns it into visibility and render layers.

use bevy::prelude::*;
use earth_two_world::terrain::{CHUNK_SIZE, TILE_SIZE};

use super::{
    merge::Merged,
    terrain::{TerrainChunk, TerrainTile},
};

/// How far away the world's small pieces are drawn, from the camera to
/// the nearest edge of each.
pub const DRAW_DISTANCE: f32 = 160.0;
/// More metres away per square metre of radius.
pub const SIGHT_RANGE: f32 = 25.0;
pub const SIGHT_MAX: f32 = 12000.0;

/// Half the width of the square round the view where things cast shadows,
/// in metres (`shadowRange` in game.go).
pub const SHADOW_RANGE: f32 = 45.0;
/// How far from the camera pieces out of view are still drawn into the
/// shadow map, so they throw their shadows into it: past the shadowed
/// square round the view, as the sun is low and the shadows long.
pub const SHADOW_REACH: f32 = 2.0 * SHADOW_RANGE;
/// The camera's clip planes (`camera.go`).
pub const CLIP_NEAR: f32 = 0.2;
pub const CLIP_FAR: f32 = 20000.0;

/// `sight`: how far away a piece of radius r is drawn.
pub fn sight(r: f32) -> f32 {
    SIGHT_MAX.min(DRAW_DISTANCE + SIGHT_RANGE * r * r)
}

/// A model's bounds as a sphere about its middle, in its own frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

/// A piece's bounds, in its own frame: what the cull looks at. The viewer
/// sets it from a model's meshes once they load; anything with one (and a
/// `GlobalTransform`) is culled as `cull` culls the world's pieces.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct PieceBounds(pub Sphere);

/// How a piece is drawn: to the camera (and into the shadow map), only into
/// the shadow map, or not at all.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Drawn {
    #[default]
    Seen,
    ShadowOnly,
    Unseen,
}

/// What the cull looks from: the camera's field of view (vertical, in
/// degrees; 0 for raylib's 45) and the window's width over its height.
/// The viewer mirrors the camera and window into it; headless tests set it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CullEye {
    pub fovy: f32,
    pub aspect: f32,
}

impl Default for CullEye {
    fn default() -> Self {
        CullEye {
            fovy: 45.0,
            aspect: 16.0 / 9.0,
        }
    }
}

/// `view`: what a camera sees: from `at`, looking `ahead`, with `up` and
/// `right` across its view; `tan_v` is the tangent of half its vertical
/// field of view, and `aspect` its width over its height.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct View {
    pub at: Vec3,
    pub ahead: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub tan_v: f32,
    pub aspect: f32,
    pub tan_h: f32,
    pub edge_h: f32,
    pub edge_v: f32,
}

impl View {
    /// The view of a camera at `eye`, as `cull` builds it.
    pub fn of(eye: &GlobalTransform, cull: &CullEye) -> View {
        let (_, rotation, translation) = eye.to_scale_rotation_translation();
        let fovy = if cull.fovy == 0.0 { 45.0 } else { cull.fovy };
        let mut v = View {
            at: translation,
            ahead: rotation * Vec3::NEG_Z,
            up: rotation * Vec3::Y,
            tan_v: ((fovy as f64) * std::f64::consts::PI / 360.0).tan() as f32,
            aspect: cull.aspect,
            ..Default::default()
        };
        v.right = v.ahead.cross(v.up);
        v.prepare();
        v
    }

    /// `prepare`: computes the side-plane slopes once per camera, rather
    /// than taking two square roots for each of the world's thousands of
    /// pieces.
    pub fn prepare(&mut self) {
        self.tan_h = self.tan_v * self.aspect;
        self.edge_h = ((1.0 + self.tan_h * self.tan_h) as f64).sqrt() as f32;
        self.edge_v = ((1.0 + self.tan_v * self.tan_v) as f64).sqrt() as f32;
    }

    /// `sees`: whether any of a sphere about `center` of `radius` is in
    /// view, no further than `far`.
    pub fn sees(&self, center: Vec3, radius: f32, far: f32) -> bool {
        let d = center - self.at;
        if d.length_squared() > (far + radius) * (far + radius) {
            return false;
        }
        let z = d.dot(self.ahead);
        if z < -radius {
            return false; // behind
        }
        // Outside a side of the view: further across than the view is wide
        // at that depth, by more than the sphere reaches (the side planes
        // slope, so a sphere touching one reaches radius/cos across).
        let mut v = *self;
        if v.edge_v == 0.0 {
            v.prepare();
        }
        d.dot(v.right).abs() - z * v.tan_h <= radius * v.edge_h
            && d.dot(v.up).abs() - z * v.tan_v <= radius * v.edge_v
    }
}

/// `chunkSphere`: covers a chunk's square and the world's relief, including
/// canyon floors (-66 m) and the high rim (200 m).
pub fn chunk_sphere(at: Vec3) -> Sphere {
    Sphere {
        center: at + Vec3::new(0.0, 60.0, 0.0),
        radius: CHUNK_SIZE * 0.9,
    }
}

/// `show`: tells the renderer how to draw `e`, if that's changed.
pub fn show(commands: &mut Commands, e: Entity, was: Option<&Drawn>, d: Drawn) {
    if was == Some(&d) {
        return;
    }
    commands.entity(e).insert(d);
}

/// `cull`: hides the world's pieces the camera can't see, so they're not
/// drawn: those behind it, off the sides of the view, or further than
/// [`DRAW_DISTANCE`]. Those out of view but near enough to throw a shadow
/// into it are drawn only into the shadow map. The terrain's tiles are
/// culled out to the horizon, but those off to the sides and behind;
/// detail meshes cast no shadows, so offscreen chunks can be skipped
/// completely, allowing for the terrain's relief as well as its XZ extent.
pub fn cull(
    mut commands: Commands,
    eyes: Query<(&GlobalTransform, &CullEye)>,
    pieces: Query<(Entity, &PieceBounds, &GlobalTransform, Option<&Drawn>)>,
    tiles: Query<(Entity, &Transform, Option<&Drawn>), With<TerrainTile>>,
    chunks: Query<(Entity, &Transform, Option<&Drawn>), With<TerrainChunk>>,
) {
    let Ok((eye, cull)) = eyes.single() else {
        return;
    };
    let v = View::of(eye, cull);
    for (e, b, g, was) in &pieces {
        let center = g.transform_point(b.0.center);
        let d = if v.sees(center, b.0.radius, sight(b.0.radius)) {
            Drawn::Seen
        } else if center.distance_squared(v.at)
            < (SHADOW_REACH + b.0.radius) * (SHADOW_REACH + b.0.radius)
        {
            Drawn::ShadowOnly
        } else {
            Drawn::Unseen
        };
        show(&mut commands, e, was, d);
    }
    for (e, tr, was) in &tiles {
        let d = if v.sees(
            tr.translation + Vec3::new(0.0, 40.0, 0.0),
            TILE_SIZE * 0.75,
            CLIP_FAR,
        ) {
            Drawn::Seen
        } else {
            Drawn::Unseen
        };
        show(&mut commands, e, was, d);
    }
    for (e, tr, was) in &chunks {
        let b = chunk_sphere(tr.translation);
        let d = if v.sees(b.center, b.radius, CLIP_FAR) {
            Drawn::Seen
        } else {
            Drawn::Unseen
        };
        show(&mut commands, e, was, d);
    }
}

/// `cullMerged`: hides the merged meshes of the world's small pieces (see
/// [`Merged`]) the camera can't see, as `cull` does the pieces: each as far
/// off as its biggest piece would be seen. The shadows merged from them are
/// drawn only into the shadow map, and only near enough to throw one into
/// it.
pub fn cull_merged(
    mut commands: Commands,
    eyes: Query<(&GlobalTransform, &CullEye)>,
    merged: Query<(Entity, &Merged, &GlobalTransform, Option<&Drawn>)>,
) {
    let Ok((eye, cull)) = eyes.single() else {
        return;
    };
    let v = View::of(eye, cull);
    for (e, m, g, was) in &merged {
        let center = g.translation() + m.center;
        let near =
            center.distance_squared(v.at) < (SHADOW_REACH + m.radius) * (SHADOW_REACH + m.radius);
        let d = if m.shadow {
            // Always shadow-only: shown is in the shadow map.
            if near { Drawn::Seen } else { Drawn::Unseen }
        } else if v.sees(center, m.radius, sight(m.piece)) {
            Drawn::Seen
        } else if m.casts && near {
            Drawn::ShadowOnly
        } else {
            Drawn::Unseen
        };
        show(&mut commands, e, was, d);
    }
}
