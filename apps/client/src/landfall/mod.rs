//! Landfall and the Red round it: the terrain streamed round the player
//! (`terrain`), the scatter on the Fringe (`scatter`), what's culled
//! (`cull`, `cull_people`), the maps' data (`maps`), what's underfoot
//! (`surface`), the light zones (`zones`), the fixed lamps (`lamps`) and
//! the merging of the small pieces' meshes (`merge`). The Go client's
//! `game/terrain.go`, `scatter.go`, `cull*.go`, `maps.go`, `surface.go`,
//! `zones.go`, `lamps.go` and `world/merge.go`, and the Landfall load path
//! of `game.go`'s `setup`.
//!
//! The decisions (what streams where, the chunk and tile maths, the
//! colliders, where the scatter stands, what's drawn) are rendering-free
//! and run headless; the meshes and textures they call for are built by
//! `render`, behind the `viewer` feature.

pub mod cull;
pub mod cull_people;
pub mod ground;
pub mod lamps;
pub mod maps;
pub mod merge;
pub mod scatter;
pub mod surface;
pub mod terrain;
pub mod zones;

#[cfg(feature = "viewer")]
pub mod render;

use bevy::prelude::*;
use earth_two_world::{
    kit::{Layout, Placement},
    terrain::ARRIVAL,
};

use crate::world::{KitAsset, LayoutAsset, LayoutRoot, SpawnLayout, assembly};

/// The rendering budgets: `game/budget_gl.go` on the desktop,
/// `budget_js.go` in the browser. Physics uses the chunks' full cells on
/// both; the browser spends less on the horizon, paint, shadows and
/// decorative scatter.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub use_high_dpi: bool,
    pub use_msaa: bool,
    pub shadow_size: u32,
    pub tile_cells: usize,
    pub detail_radius: i32,
    pub chunk_texels: usize,
    pub tile_texels: usize,
    pub chunks_per_frame: usize,
    pub scatter_radius: i32,
    pub simulate_cloth: bool,
}

impl Budget {
    pub const DESKTOP: Budget = Budget {
        use_high_dpi: true,
        use_msaa: true,
        shadow_size: 4096,
        tile_cells: 64,
        detail_radius: 3,
        chunk_texels: 128,
        tile_texels: 64,
        chunks_per_frame: 2,
        scatter_radius: 3,
        simulate_cloth: true,
    };

    pub const BROWSER: Budget = Budget {
        use_high_dpi: false,
        use_msaa: false,
        shadow_size: 1024,
        tile_cells: 16,
        detail_radius: 2,
        chunk_texels: 64,
        tile_texels: 32,
        chunks_per_frame: 1,
        scatter_radius: 2,
        simulate_cloth: false,
    };

    /// The budget this build runs with.
    pub fn platform() -> Budget {
        if cfg!(target_arch = "wasm32") {
            Budget::BROWSER
        } else {
            Budget::DESKTOP
        }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Budget::platform()
    }
}

/// What the terrain and the scatter stream round: the player
/// (`character.Player` in the Go client). The character port marks the
/// player's root with it.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct StreamCentre;

/// The sets the subsystem's systems run in, for the other ports to order
/// against: `game.WorldStream` ran after `character.Act` in the Go client,
/// and the culling after it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LandfallSet {
    /// `streamTerrain` and `streamScatter`.
    Stream,
    /// `cull`, `cullMerged` and `cullPeople`.
    Cull,
}

/// Asks for Landfall to be placed as `setup` places it: the terrain round
/// the arrival, and `world/landfall.json` stood on it, with the map, the
/// soundscape and the fixed lamps read from the same placements.
#[derive(Component, Debug)]
pub struct SpawnLandfall {
    pub layout: SpawnLayout,
}

impl SpawnLandfall {
    pub fn load(assets: &AssetServer) -> SpawnLandfall {
        SpawnLandfall {
            layout: SpawnLayout::load(assets, "world/world.json", "world/landfall.json"),
        }
    }
}

/// The subsystem: streaming, scatter, culling and the Landfall load path.
#[derive(Default)]
pub struct LandfallPlugin {
    pub budget: Budget,
}

impl Plugin for LandfallPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.budget)
            .init_resource::<scatter::Scatter>()
            .configure_sets(Update, (LandfallSet::Stream, LandfallSet::Cull).chain())
            .add_systems(Update, place_landfall)
            .add_systems(
                Update,
                (terrain::stream_terrain, scatter::stream_scatter)
                    .chain()
                    .in_set(LandfallSet::Stream),
            )
            .add_systems(
                Update,
                (cull::cull, cull::cull_merged, cull_people::cull_people)
                    .chain()
                    .in_set(LandfallSet::Cull),
            );
        #[cfg(feature = "viewer")]
        app.add_plugins(render::LandfallRenderPlugin);
    }
}

/// `spawnOnTerrain`'s standing: each placement lifted by `standOn`, so a
/// piece on a slope sinks into the high side rather than floating off the
/// low one (on the level, in town, it's where the layout put it).
pub fn stood_on_terrain(kit: &earth_two_world::kit::Kit, layout: &Layout) -> Layout {
    Layout {
        pieces: layout
            .pieces
            .iter()
            .map(|p| {
                let mut p: Placement = p.clone();
                p.at[1] += terrain::stand_on(kit, &p);
                p
            })
            .collect(),
    }
}

/// Places Landfall once its files load, as `setup` does: the terrain round
/// the arrival, then the layout stood on it, the map, the soundscape, the
/// fixed lamps and the merge plan the viewer builds from.
fn place_landfall(
    mut commands: Commands,
    requests: Query<(Entity, &SpawnLandfall)>,
    kits: Res<Assets<KitAsset>>,
    layouts: Res<Assets<LayoutAsset>>,
    assets: Res<AssetServer>,
    budget: Res<Budget>,
) {
    for (entity, request) in &requests {
        let (Some(kit), Some(layout)) = (
            kits.get(&request.layout.kit),
            layouts.get(&request.layout.layout),
        ) else {
            if assets.load_state(&request.layout.kit).is_failed()
                || assets.load_state(&request.layout.layout).is_failed()
            {
                error!("landfall: could not load world/world.json or world/landfall.json");
                commands.entity(entity).despawn();
            }
            continue;
        };
        commands.entity(entity).despawn();
        let kit = &kit.0;
        let placed = &layout.0;
        // The Red under the seats and out across the Fringe, round where the
        // player arrives. Floors have no colliders of their own: this is
        // what everyone walks on.
        let terrain = terrain::spawn_terrain(&mut commands, &budget, ARRIVAL);
        commands.insert_resource(terrain);
        let stood = stood_on_terrain(kit, placed);
        match assembly(kit, &stood) {
            Ok(pieces) => {
                let root = LayoutRoot {
                    layout: "world/landfall.json".into(),
                };
                commands.spawn_scene(bsn! {
                    Name::new("world/landfall.json")
                    root
                    Transform::default()
                    Children [ {pieces} ]
                });
            }
            Err(err) => {
                error!("{err}");
                continue;
            }
        }
        commands.insert_resource(maps::WorldMap::new(kit, &placed.pieces));
        commands.insert_resource(surface::Soundscape::new(kit, &placed.pieces));
        match lamps::fixed_lamps_of(kit, &placed.pieces) {
            Ok(lamps) => commands.insert_resource(lamps::FixedLamps(lamps)),
            Err(err) => error!("{err}"),
        }
        commands.insert_resource(merge::MergeRequest {
            placements: stood.pieces.clone(),
        });
    }
}
