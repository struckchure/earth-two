# landfall

World streaming: the Go client's `game/terrain.go` (after the height model,
which is `crates/world`'s), `surface.go`, `scatter.go`, `cull.go`,
`cull_people.go`, `maps.go` (data and selection; drawing is a later port),
`zones.go`, `lamps.go`, `world/merge.go` and the Landfall load path of
`game.go`'s `setup`.

| File | Go | What it owns |
| --- | --- | --- |
| `mod.rs` | `budget_*.go`, `setup` | `Budget` (desktop/browser), `LandfallPlugin`, `SpawnLandfall`, `StreamCentre`, `LandfallSet` |
| `terrain.rs` | `terrain.go` | chunk colliders (Avian trimeshes), tiles, `Terrain` streaming, `stand_on`, `terrain_around` |
| `scatter.rs` | `scatter.go` | seeded cell contents, `scatter_pose`, streaming |
| `cull.rs` | `cull.go` | `View`, `sight`, `Drawn`, piece/tile/chunk/merged culls |
| `cull_people.rs` | `cull_people.go` | the rules per person and the `PeopleCull` hook |
| `maps.rs` | `maps.go` | marks, index, `marks_near`, `MapFrame`, clicks, headings |
| `surface.rs` | `surface.go` | `Footing`, `Soundscape` |
| `zones.rs` | `zones.go` | light zones, `places_at`, `light_at` |
| `lamps.rs` | `lamps.go` | `fixed_lamps` |
| `merge.rs` | `merge.go` | rendering-free merge plan, `Merged` |
| `ground.rs` | private helpers of `terrain.go`/`maps.go` | `Rectangle`, `footprint`, `level_distance` |
| `render.rs` (viewer) | `shape`, `paint`, `PlaceMerged` | meshes, paint, bounds, `Drawn` to visibility, merging |

Everything but `render.rs` compiles and runs headless. The streaming
systems mark entities (`ChunkRebuild`, `TileResink`, `Drawn`) and the
viewer reacts; without the viewer the marks are inert.

Hooks the other ports provide: the player's root carries `StreamCentre`;
the camera carries `CullEye` (the viewer fills it from `Camera3d`); the
sun's `RenderLayers` include `render::SHADOW_LAYER`; the character port
installs a `PeopleCuller`. The playable game now installs this adapter in
`game/people.rs`; it applies the existing rules to character roots, bodies and
all loaded mesh descendants.
