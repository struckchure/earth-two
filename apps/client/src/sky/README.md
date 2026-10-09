# Sky and atmosphere

Port of `game/{sky,skytex,daylight,clock,stars,stars_budget*,weather,effects*}.go`.
`game/ambience.go` is audio and is not here.

Add `sky::SkyPlugin`. Its `Update` systems run in the `sky::SkySystems` set, in
Go's order: `turn_sun` (daylight.rs), then `blow` (weather.rs), then
`move_effects` (effects.rs). Anything reading the resources below should be
ordered `.after(SkySystems)`.

## Resources other systems read

| Resource | Fields | Go source |
| --- | --- | --- |
| `Clock` | `hour: f32` — the hour the sun's held at (EARTH_TWO_HOUR), `-1` to follow the real clock; `now()`/`at(real)` give the moment the sun is at | `daylight.hour`, `now()` |
| `Daylight` | `now: UnixTime`; `sun: Vec3` (unit way to the sun, Go `sunFrom`); `light: Vec3` (way the scene's light comes from, never under the ground); `dusk`, `night: f32` (0–1); `turn: f32` (how far the stars' sky has turned, radians); `baked: SkyPaint`; `lamps_on()` (Go's `vehicle.LightCycle.Night`: `night > .01 \|\| dusk > .75`) | `daylight`, `turnSun`, `lightFrom` |
| `Weather` | `storm: f32` (0–1, dusty from `DUSTY_FROM` .15, a storm from `STORM_FROM` .6); `forced: f32` (held storm, EARTH_TWO_STORM, or −1; `hold()`); `t: f32` (seconds the blown dust and the stars' twinkle run on) | `weather` |
| `Haze` | `color: Rgba`, `distance: f32` (share 1 − 1/e of the way), `end: f32`, `veil: f32` — the fog the shaders draw: `mix(colour, haze, d < end ? 1 - exp(-d/distance) : veil)` | `shading.Haze`, set by `blow` |
| `Ambient` | `color: Rgba`, `brightness: f32` — the fill from the sky, in the Go shader's scale (0.26 clear) | `render.AmbientLight` |
| `SunLight` | `color: Rgba`, `brightness: f32` — the sun's light, Go's scale (1.3 clear, past white) | `render.DirectionalLight` |
| `Effects` | the particles (`puff`, `ring`, `trail`, `wheel`, `sparks`), their wind | `effects` |

Colours are `sky::Rgba`, sRGB bytes as Go kept them (`to_bevy()` / `to_linear()`
with the viewer). The clear-day haze/ambient defaults are the Go plugin's
(`skyHorizon`, 1600, `bodyDistance − 200`; `(206,186,222)`, 0.26); `blow` takes
whatever `Haze`/`Ambient` hold on its first run as the clear day, so a shading
plugin that inserts its own before `Startup` is honoured.

The `Sun` marker component is on the entity whose `Transform` looks the way the
light goes (`-light` forward); with the viewer, `render::sync_light` puts a
`DirectionalLight` on it, sets `GlobalAmbientLight`, and gives every `Camera3d`
a `DistanceFog` (exponential, density `1/distance`) as a stand-in until the
shading port draws the haze itself. `SUN_LUX_PER_UNIT` and `AMBIENT_PER_UNIT`
are the stand-in scales; they claim no parity.

## Headless

`clock`, `sun` (sun path, dusk/night, ecliptic and neighbours, sky colours,
glare), `stars` (catalogue, Milky Way, `visible_stars`), `skytex` (`paint_sky`
and the surfaces as pixel buffers), `weather` (`storm_at`, `blow_to`), `effects`
(motion and `quads`) and `time` (`UnixTime`, UTC nanoseconds like Go's
`UnixNano`) all build with `--no-default-features`.

## Viewer (`render.rs`)

The dome (our own Y-poled longitude-latitude sphere, so the texture is painted
by direction rather than through raylib's sphere UVs), the sun disc, the six
neighbours, the stars as a line-list mesh rebuilt each frame (budget and
crosses per `stars.rs`, wasm gets 2500 dots), the particles as one quad mesh
with a soft-disc texture (the Go fragment shader's falloff), the blown dust as
UI nodes under `GlobalZIndex(-10)`, repainting the sky on an
`AsyncComputeTaskPool` task as Go did on a goroutine, and `far_enough`, which
raises any `Camera3d` perspective far plane to `SKY_FAR` so the dome is seen.

`cargo run --example sky` captures `build/rust/sky-11.png` and
`build/rust/sky-22.png`; with `EARTH_TWO_HOUR` set, `build/rust/sky.png`.
