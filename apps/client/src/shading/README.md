# shading

The game's look, ported from the Go `shading` package: toon shading for
everything, ink outlines for what asks for them.

| Go | Rust |
| --- | --- |
| `shading/plugin.go` (settings, uniforms, haze) | `mod.rs`: `Look`, `Haze`, `ShadingPlugin`; `plugin.rs`: `painted_uniform`, `sync_materials` |
| `shading/toon.go` (`toonSurface` GLSL) | `painted.wesl` fragment |
| `shading/lamps.go` (`lampGLSL`, `groupLamps`) | `lamp_light`, `lamp_point_light`; `world_lamps` in `painted.wesl` over Bevy's clustered point lights (every lamp, no budget) |
| `shading/spotlight.go` (`gatherSpots`) | `SpotLight`, `BeamEye`, `pick_beams`, `gather_spots` |
| `shading/outline.go` (`outlineVertex`, `outlineFragment`) | `outline.wesl`; `OutlineMaterial`, `Outlined`, `OutlineHull` |
| `shading/lighting_gl.go` / `lighting_js.go` + illusion `render/shader.go` (`sunShadow`, `pointLight`) | `sun_shadow` and `world_lamps` in `painted.wesl`, on Bevy's shadow maps and clusters |
| illusion `render/shadow.go` (`Shadows{Size, Range}`) | `ShadowQuality`, `sun_cascades`, `apply_shadow_quality` |
| `game/zones.go` | `Zone::weight`, `places_at`, `Look::light_at` |
| `game/game.go` plugin config, `game/lamps.go` | `Look::earth_two`, `light_zones`, `attach_lamps` |

`boxOutlineVertex` (outlines on `render.Cuboid`) has no Rust counterpart
yet: nothing in the client draws cuboids with outlines.

## Using it

Add `ShadingPlugin::earth_two()` after `DefaultPlugins`. Every standard
material that appears is replaced by a `PaintedMaterial`; a `Smooth` mesh
(the ground) is lit smoothly; meshes under an `Outlined` entity get a hull
child drawn with `OutlineMaterial`. `world::Lamp` entities get their
point light. Spawn the sun as a `DirectionalLight` with `sun_cascades()`;
its transform gives the shader its direction, and `Daylight` its colour.
`ShadowQuality` (Full/Low/Off) sets the map size (4096 desktop, 1024
browser), the filter and whether the sun casts at all.

Cameras are set to `Tonemapping::None` and `DebandDither::Disabled`: the
shader does its maths on sRGB-encoded values, as the Go one did, and
decodes its result.
