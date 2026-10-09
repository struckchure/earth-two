# Rust migration with Bevy and Avian

Earth Two will migrate from Go, illusion, raylib, Ark ECS and Jolt to Rust,
Bevy **0.20.0** and Avian 3D, with BSN for scene composition. The migration must preserve the existing game:
its appearance, controls, movement, vehicle handling, content, account data,
desktop and browser support, and release workflows. Performance improvements
must be measured at equivalent quality and workload.

Status: the Step 3 local review checkpoint opens a playable application with
Landfall, a player, walking/traversal, vehicle entry/driving/exit, and a
loading/title/play/pause lifecycle. The foundation viewer remains available
with `--foundation`. There are 261 passing headless tests; the render-side
terrain regression and native graphical game smoke also pass. Visual and
handling parity, full gameplay/UI, and the release-platform matrix remain
open. Go remains the release default.
A working module or green unit test is not evidence of integrated game parity.
The completion plan below is the execution order from this checkpoint.

The reference for “the game today” is the Go implementation at
`c561bd69e06500aa3c79e9bb0d0d5e974694e7c7`. The inherited Rust work and
continuation fixes were preserved separately in local commit `870a89c`. Capture
its asset hashes, dependency locks and build configuration along with the source. Features described only in the world
bible are outside scope. If main changes during the migration, inventory
those changes explicitly and update the reference scenarios before cutover.

## Continuation checkpoint: 2026-10-09

Continued Claude's uncommitted `silent-ibis` / `golang-rust` work in place.
The source has advanced beyond the foundation status above: character
movement/traversal, presentation/wardrobe/cloth, vehicles, Landfall streaming,
sky/weather, shading and identity/accounts have modules and tests. This is
implementation coverage, not accepted gameplay or visual parity; the default
viewer still shows the foundation scene, and the full game flow remains open.

This continuation repaired the presentation example's missing `viewer`
feature gate and the Bevy 0.20 spotlight field name. Parked vehicles opt out
of transform interpolation until driven, including after re-parking. Paused
root bodies now present their exact last simulated pose despite a continuing
render clock; the regression checks position, rotation and resumed movement.
Vehicle exit clearance confirms signed penetration rather than rejecting a
capsule on a conservative intersection result: the large flat floor produced
contacts with negative penetration (about 2 cm separation). The headlamp
on/off/exit scenarios now pass without changing their expected behavior.
The gearbox regression holds RPM within the shift band while waiting for
clutch latency; previously it unintentionally requested another upshift.
Formatting and Clippy findings in the unfinished ports were also resolved.

Validation on macOS arm64 with the pinned toolchain:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Pass |
| `cargo check --locked -p earth-two-client --all-targets` | Pass |
| `cargo check --locked -p earth-two-client --target wasm32-unknown-unknown` | Pass; browser execution not tested |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --workspace --no-default-features --no-fail-fast` | 238 passed, 2 failed; no ignored tests |

The remaining failures are in `apps/client/tests/vehicle.rs`:

- `real_vehicles_stay_upright`: the bike reaches minimum up.Y around 0.74,
  below the existing 0.8 requirement during hard cornering.
- `real_vehicles_keep_normal_pedestrian_response_with_crowd_lod`: the bike
  deviates from its initial straight route and misses the pedestrian. In the
  observed run, the car ends near (-13.18, 0.001, 49.95) and the pedestrian
  remains near (-0.89, 0.92, 9.51). Both full-rate and reduced-rate cases must
  be investigated; this does not yet establish a crowd LOD bug.

Both tests stop at the bike, so their later vehicle cases remain unverified.
Keep their assertions and the Go reference thresholds intact. The next task
is to isolate wheel contact geometry, straight-line drift and motorcycle
lean response before integrating vehicle interaction into a playable scene.
The suite is still red; do not mark the physics/parity gates complete.
Generated logs are under `build/migration-baseline/continuation-2026-10-09/`
(ignored), including the initial and final headless runs. No Go reference
baseline, browser runtime capture or cross-platform runtime evidence was
added in this continuation.

## Step 1 local review checkpoint: 2026-10-09

This is the first reviewable baseline increment, not completion of the full
cross-platform Step 1 exit gate. No game behavior or Go release default changes
in this increment. Review it before starting the vehicle fixes in Step 2.

The target authority is `.github/workflows/ci.yml`: Linux amd64 on
`ubuntu-24.04`, macOS arm64 on `macos-15`, macOS amd64 on `macos-15-intel`,
Windows amd64 on `windows-latest`, and web using Emscripten `6.0.10` on
`ubuntu-latest`. CI builds web but does not select browser engines or run
browser gameplay tests. Browser coverage remains an explicit open item.

`tools/migration/scenarios.json` defines 21 source-linked acceptance scenarios,
including setup, expected behavior and evidence requirements. The baseline
runner extracts the pinned Go commit into an isolated directory, records
asset/dependency/source hashes and test inventory, sanitizes game environment
overrides, and uses a disposable identity path. Existing evidence directories
are never reused. Generated evidence stays under ignored `build/`.

Local evidence is in `build/migration-baseline/step-1-go-reference-v2/`:

- `test-summary.json`: 420 passing Go test/subtest events, zero failures;
  17 passing packages. Parent and subtest events are counted separately.
  Three initial skips include two live-database tests and the packed-assets
  test; the separate `packed-assets` run passed. The database cases remain
  unverified.
- `benchmarks.stdout.log`: five repeats of existing terrain, minimap and
  visibility CPU benchmarks. These are not whole-game performance acceptance.
- `web-run.json` and `web.*.log`: original web release build passed using
  local Emscripten `6.0.10-git`, including the account bridge and compression.
  `web-title.png` records normal startup and `web-startup.png` records initial
  benchmark rendering in the automation browser;
  its benchmark was still warming up, so no browser performance result is
  claimed. Browser version/backend and gameplay coverage remain unverified.
- `earth-two-reference`: built native Go reference; source/assets and prepared
  dependency workspace are under `reference/`.
- `traces/`: three repeats of the same 600-tick, 60 Hz driving script for bike,
  buggy, hauler, tanker, rover and trike. All six had identical per-tick
  positions across these local Go runs. `rust-traces/1/` holds matching Rust
  diagnostics. The bike's minimum up.Y is about 0.837 in Go versus 0.738 in
  Rust. Rig equivalence and other state differences still require analysis;
  zero local variance does not establish a cross-engine zero tolerance.
- `review-captures/`: 36 normal desktop tour captures (12 views in day, night
  and storm conditions, each 2560×1440) and their source/settings,
  hashes and image dimensions. Forced map capture is excluded: the original
  tour selects that screen after the map-input initialization phase, leaving
  zoom zero on its first draw and stalling the tour. The interrupted original
  run and its first ten images are preserved under `captures/day/`. This does
  not establish a fault in normal map interaction, which remains unverified.

Reproduce the baseline from this worktree:

```sh
make migration-tools-test
make migration-baseline BASELINE_ARGS='--capture --web'
# Use the evidence directory printed by the previous command:
python3 tools/migration/trace.py build/migration-baseline/<run>
EARTH_TWO_RUST_TRACE_OUT="$PWD/build/migration-baseline/<run>/rust-traces/1" \
  cargo test --locked -p earth-two-client --no-default-features \
  --test vehicle migration_vehicle_trace -- --exact
```

To review the frozen Go game already built here:

```sh
cd build/migration-baseline/step-1-go-reference-v2/reference
EARTH_TWO_IDENTITY_PATH="$PWD/../review-identity/pk" ../earth-two-reference
```

Validation of this increment: the five Python evidence-tool tests pass;
`cargo fmt --all --check` and workspace Clippy with warnings denied pass;
the complete Rust headless suite reports 239 passed, two failed, zero ignored.
Both failures are the previously documented vehicle cases.

Full interactive route clips, locomotion/animation/event traces, account/server
integration, audio, alternate render-rate/focus/streaming stress, memory/GPU
measurements, paired visual tolerances and remote platform runtime evidence
remain pending. These must be added alongside the relevant subsystem increments
before final parity acceptance; the current artifacts do not prove the Rust
client plays like the Go game. The Rust executable still opens its foundation
viewer, and the two existing vehicle failures remain visible.

## Step 3 local review checkpoint: 2026-10-09

The Rust executable now assembles the existing subsystem ports into the first
playable slice. `game::GamePlugin` owns loading/title/playing/paused states,
player creation, input and interaction ordering, seats, orbit/follow/collision
camera, stream centre, nearby vehicle ground coverage and the sky/shading
light bridge. World placements remain the authored BSN assemblies. Wardrobe
JSON and character/world models load asynchronously through AssetServer on
both platforms. Failed assets/dependencies and malformed wardrobe data are
shown in the loading panel.

The Go main menu preserves its current player and world. This port therefore
resumes the same session on Play; it does not invent a fresh-world reset.
Physics and gameplay controls pause in menus. Tests repeat this cycle five
times and check that one player and the same layout roots remain. Vehicle
entry removes the character capsule, seats the player at the authored pose,
and exits restore the standing controller. Hand/foot IK and complete ride
poses remain Step 4 work.

Integration exposed three render problems: a reserved shader identifier,
terrain rebuilds that stopped when standard materials became painted
materials, and world batching that lost its source-material metadata. Those
are fixed. Ground uses the smooth painted material, batching retains source
metadata, and the review block is excluded from Landfall's merge. Native
Landfall batches into 1,027 meshes. Browser terrain/scatter retains the
existing lower platform budget.

From `silent-ibis`:

```sh
cargo run --locked -p earth-two-client
cargo run --locked -p earth-two-client -- --at=buggy
cargo run --locked -p earth-two-client -- --at=traversal
cargo run --locked -p earth-two-client -- --foundation
```

The default spawn is Go's Landfall arrival. The buggy option starts beside
an authored parked buggy. The traversal option adds the exact Hull test
block placements above the live terrain at y=40, with its test floor; this
is an opt-in review fixture, not new default game content. Its placement
manifest is embedded because Go's release pack excludes development layouts.
Models and textures still come from the selected asset root.

Click Play or press Enter (focus the canvas first in a browser). WASD/arrows
move; Shift runs; Space jumps/traverses; C crouches; Ctrl slides; R rolls.
E enters/exits a nearby vehicle. Driving uses WASD/arrows, Space handbrake,
H headlamps and R recovery. Mouse movement orbits on desktop; hold either
mouse button and drag in the browser. Esc pauses/resumes; arrows and Enter
select Resume/Main menu. Returning to the main menu retains the session.

Reproduce the browser build with Python 3.11+:

```sh
python3.11 tools/rust/web.py --assets build/migration-baseline/step-1-go-reference-v2/reference/build/assets
python3 -m http.server 8087 --bind 127.0.0.1 --directory build/rust-web
```

Open `http://127.0.0.1:8087/`, `/?at=buggy`, `/?at=traversal`, or
`/?foundation`. `EARTH_TWO_ASSET_ROOT` selects a native asset directory.

Evidence lives in `build/migration-baseline/step-3-playable/` (ignored):

- `tests.log`: 261 headless tests pass, including keyboard walking,
  enter/drive/brake/exit with capsule restoration, repeated menu transitions,
  camera turn/recentering constants, and a keyboard-triggered vault over the
  real Hull crate (fixture initially faces the crate, as in Go's test).
- `terrain-render-test.log`: relocated terrain updates its mesh and replaces
  its texture both before and after painted-material adoption.
- `native-title.log` / `title.png`: macOS arm64 Metal loads 3,800 model scenes
  and the player. `native-smoke.log` / `native-play.png` run a synthetic-input
  graphical route through play, enter, drive, exit, walk, pause, title and
  resume. Run it with `cargo run --locked -p earth-two-client --example game_smoke`.
  This example injects game inputs and controls focus/cursor state for the
  test; it does not send OS keyboard events.
- `web-build-final.log`: optimized WASM and packed-asset packaging pass.
  The dedicated Chromium/SwiftShader WebGL2 run loads 3,464 model scenes.
  Real browser keyboard input exercises Play, buggy entry, throttle with
  ArrowUp, exit, walking, pause and return to title. Captures include
  `browser-seat.png`, `browser-arrow-driving.png`, `browser-exit.png`,
  `browser-pause-menu.png` and `browser-title-after-play.png`.
  `browser-errors.log` reports no JavaScript errors. This software adapter
  is slow and does not establish browser hardware-performance parity.
  The final build also loads 3,586 scenes with the Hull review fixture;
  `browser-traversal.png` and `browser-traversal-action.png` show the block
  and a keyboard Space action. The exact vault endpoint is asserted in the
  headless integration test; browser animation/pose parity is still open.
  `browser-final-errors.log` reports no JavaScript errors after this run.
- `clippy-final.log`: workspace/all-target Clippy with warnings denied passes;
  formatting and whitespace checks pass too.

This is a gameplay integration checkpoint, not visual parity approval. Current
captures visibly show clothing/skin fitting and animation attachment issues,
lighting/shadow differences, and a plain temporary menu/HUD. Full title/pause
settings, wardrobe/account UI, sound, residents, injuries and remaining
interactions are still covered by Steps 4–6. The Step 2 handling differences
remain open. Native macOS arm64 and a software WebGL2 browser are local
coverage only; Linux, Windows, macOS x64 and additional browser engines still
need runtime validation. Stop here for review before starting Step 4.

## Step 2 local review checkpoint: 2026-10-09

This increment repairs the vehicle blockers and supplies a drivable physics
review scene. It does not complete the cross-platform parity gate or assemble
the game lifecycle from Step 3. Go remains the release default.

The implementation follows the frozen Go vehicle package and its pinned
illusion/Jolt adapter, without changing the authored handling constants or
weakening the original assertions:

- Replace the overlapping chassis boxes with the authored convex hull and
  its inertia shifted to the authored centre of mass. Retain Jolt's default
  linear/angular damping and the rounded wheel-cast geometry.
- Remove parked tyre colliders before simulation, and exclude a vehicle's
  own tyres from casts. Their old lifecycle caused the initial motorcycle kick.
- Solve longitudinal tyre and motorcycle lean impulses in each existing tyre
  iteration. Apply equal/opposite wheel forces to ordinary dynamic props.
- Preserve illusion's character layer rules: wheel casts ignore characters;
  vehicle/character contact constraints make only the vehicle effectively
  infinite-mass. Walls, props and other vehicles retain ordinary response.
- Predict run-over contacts before character movement, as Go does. Prepare
  body mass/inertia before applying vehicle forces. The native review scene
  exposed an immediate-entry NaN that the old settled fixtures missed; a
  regression now covers entering all six vehicles immediately after assembly.

Each real vehicle now has independent cornering and crowd-LOD pedestrian
cases, so a bike failure cannot hide later cases. Diagnostics record contact
points/normals, suspension and tyre impulses, lean response, mass, inertia,
centre of mass and consumed inputs. Added regressions cover waking a parked
bike, dynamic prop momentum, fixed inputs at 30/60/97 Hz and alternating
20/144 Hz rendering, immediate entry, recovery teleport interpolation, and
CCD against a 10 cm wall at 1,800 m/s. Cadence coverage is at the consumed
physics-input boundary; application input sampling remains Step 3 work.

From `silent-ibis`, review any of the six authored vehicles:

```sh
cargo run --locked -p earth-two-client --example vehicle -- bike
# Also: buggy, trike, rover, hauler, hauler_tanker
```

W/S accelerates/brakes/reverses; A/D steers; Space is the handbrake; H switches
headlamps; hold R when tipped to recover; Esc pauses/resumes physics. The
example is a flat physics test scene with the original models, default
materials, a following camera and diagnostic HUD. Rider presentation,
Landfall, menus and game flow are not integrated here.

Evidence is under `build/migration-baseline/step-2-vehicles/` (ignored).
`workspace-tests-final.log` records 257 passing tests with no failures or
ignored cases, including all 37 vehicle tests. The matching Go upright,
pedestrian and solid-obstacle tests were rerun successfully; their log is
`../step-1-go-reference-v2/step2-go-vehicles.log`.

Three final Rust repeats (`final-rust/{1,2,3}/`) had identical per-tick
positions on this host. `trace-comparison.json` compares their 600-tick script
against the frozen Go runs. This is diagnostic evidence, not an accepted
cross-engine tolerance:

| Vehicle | Min up.Y Go / Rust | Final speed Go / Rust (m/s) |
| --- | --- | --- |
| bike | 0.837 / 0.809 | 17.93 / 17.22 |
| buggy | 1.000 / 0.999 | 24.03 / 23.81 |
| hauler | 0.999 / 0.999 | 7.70 / 7.50 |
| hauler_tanker | 0.999 / 0.999 | 7.66 / 7.53 |
| rover | 1.000 / 0.999 | 17.14 / 17.26 |
| trike | 0.988 / 0.983 | 13.61 / 13.87 |

The bike now exceeds the unchanged 0.8 upright threshold (previously about
0.738), but its final scripted position remains about 12.4 m from Go after
ten seconds of driving. Exact route/handling parity is not claimed.

Native macOS arm64/Metal startup and original model loading passed for all
six vehicle choices (`*-review.png`). The review camera scales to the chassis
bounds so trucks fit. `foundation-packed.png` captures 124 loaded model scenes
using the unchanged frozen Go release pack, including external texture data.
The viewer embeds its diagnostic Hull placement manifest because Go's release
packer omits that development-only file; all model/texture loads still come
from the selected asset root. Both original and packed asset runs report
`Foundation ready: 124 loaded scenes`, with the physics probe settled near
y=0.48 (`foundation-source.log`, `foundation-packed-final.log`). Workspace
Clippy with warnings denied and formatting checks pass.

The optimized WASM package also runs in the dedicated Chromium test browser:
`browser-console.log` records 124 loaded scenes and the physics probe settled
at y=0.5000; `browser-errors.log` reports no JavaScript errors.
`browser-foundation.png` shows the loaded Hull and textured assets.
`web-http.log` records successful packed model/resource requests (the optional
favicon returns 404). The adapter is software SwiftShader through WebGL2;
unsupported optional GPU effects use Bevy's documented fallbacks. Browser
version/device coverage, hardware performance, vehicle driving and visual
parity remain unverified.

Rebuild this browser checkpoint with Python 3.11 or newer:

```sh
python3.11 tools/rust/web.py --assets build/migration-baseline/step-1-go-reference-v2/reference/build/assets
python3 -m http.server 8080 --bind 127.0.0.1 --directory build/rust-web
```

Open `http://127.0.0.1:8080` for the foundation scene. The native `vehicle`
example remains the driving review command for this increment.

Handling parity is still open. Avian and Jolt use different suspension/contact
solvers; the current Rust spring model and six tyre iterations are not an
exact Jolt constraint solver port. Wheel contacts with other drivable vehicles
still lack the moving-ground reaction used for ordinary dynamic props. The
full native CI matrix (Linux amd64, macOS arm64/amd64, Windows amd64), browser
engine/device matrix, audio activation/codecs, and full gameplay/visual
comparisons must pass before this milestone's complete exit gate is accepted.

## Dependencies and the first implementation gate

Pin Bevy as `bevy = "=0.20.0"` and commit the client Cargo lockfile. Bevy
0.20 was released on October 8, 2026; its release manifest requires Rust
1.97.1 or newer. Use Rust 1.98.1 as the initial client toolchain, which is
installed in the development environment. The root workspace owns this pin;
the existing server remains an excluded independent Cargo workspace with
its own dependency lockfile.

Avian's upstream compatibility PR 1070 is pinned at
`34c9b008d2f40638a953d9218db48d54f661365d`, including its pinned companion
crate revisions. It compiles against final Bevy 0.20.0 with no local Avian
patch. The lockfile resolves the Bevy core crates to 0.20.0. Headless tests
cover BSN spawning, gravity/contact, pause/resume and ray queries. Native
Metal rendering and loading the three sample GLBs were verified on macOS
arm64, and the browser target compiles. Browser execution, the remaining
desktop matrix, CCD and interpolation-specific regression tests still need
evidence before the full gate below can be marked complete.

For future dependency updates, compile the selected Bevy 0.20-compatible
release or commit, including its transitive Bevy dependencies. Prefer a
published release; otherwise pin an exact verified revision and document
its maintenance cost. If none is available, patch Avian for Bevy 0.20 as part
of the foundation work; this is an accepted implementation path. Start from
an exact upstream revision, keep the compatibility changes isolated and
reviewable in a local vendored copy or pinned fork, preserve the license,
and record the diff and upstream base. Check companion crates as well as
Avian itself. Prefer upstreaming the patch when practical, but do not make
upstream release timing a prerequisite for progress. The patch effort will
be established by compilation and physics tests. Keep Bevy 0.20.0 and Avian as the chosen target;
do not silently downgrade Bevy or replace the physics engine.

The gate passes when native and `wasm32-unknown-unknown` builds resolve a
single compatible Bevy dependency family, and a fixed-step Avian collision
smoke scenario runs on desktop and in a browser. A manifest edit or successful
dependency resolution alone does not pass the gate.

Sources:

- [Bevy 0.20 announcement](https://bevy.org/news/bevy-0-20/)
- [Bevy 0.20.0 manifest](https://github.com/bevyengine/bevy/blob/v0.20.0/Cargo.toml)
- [Inspected Avian manifest](https://github.com/avianphysics/avian/blob/a35a80bd13cce8c61f735210fecd8a322bf9da65/crates/avian3d/Cargo.toml)
- [Pinned Avian compatibility port](https://github.com/avianphysics/avian/pull/1070)
- [Bevy 0.20 BSN composition example](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/scene/bsn.rs)

## What parity means

Content, controls, UI text, gameplay rules, stable asset identifiers and
external data formats must remain equivalent. Existing keys and encrypted
backups must work across Go and Rust clients without creating a new account.
Preserve the current SpacetimeDB schema and authorization protocol.

Jolt and Avian will not produce bit-identical physics trajectories. Compare
observable outcomes and short recorded trajectories using documented
position, rotation, velocity and timing tolerances. Establish those tolerances
from the reference behavior before tuning the Rust implementation. Threshold
decisions, such as whether a vault succeeds or an impact causes injury, must
still match. Do not enlarge tolerances to conceal a regression.

Use paired screenshots and clips at matching camera, resolution, time,
weather, wardrobe and quality settings. Check color space, tone mapping,
shadow coverage, outline width, transparency, text layout and animation
timing. Automated image differences supplement visual review; neither a
default Bevy material nor a changed lighting style establishes parity.

Preserve physics at 60 Hz, pause and resume semantics, fixed-step input
consumption, and render interpolation. Explicitly map scheduling dependencies
from illusion to Bevy, including traversal before locomotion, vehicle input
before character actions, physics writeback before impact processing, and
pose changes before bone attachment and transform propagation.

## Source inventory and acceptance scenarios

The following is the initial subsystem checklist. Expand each row into
individual test cases while recording the Go reference. Existing tests are
specifications to port and compare, not evidence that the Rust version passes.

| Area | Current implementation | Required evidence |
| --- | --- | --- |
| World and assets | `world/`, `assetref/`, `game/assets*`, `game/terrain*`, `game/scatter*`, `game/maps*`, `tools/assetpack/` | Same Landfall layout, terrain heights and collisions, scatter placement, map coordinates, logical IDs and pack fingerprints; packed GLBs load with their external `_packed/` resources. |
| Movement | `character/locomotion.go`, `character/traversal.go`, corresponding tests | Walk/run acceleration and braking, turning, jump, crouch clearance, stairs, slide, roll, vault, mantle, ladders, wall kicks, blocked routes and pause/resume. |
| Character presentation | `character/animate*`, `skin*`, `outfit*`, `cloth*`, `contacts*`, `footwear*`, `seat*`, `ride*` | Same clip selection and timing, clothing, bone transforms, feet on terrain, seating, steering poses and desktop cloth. |
| Vehicles | `vehicle/`, especially `handling.go`, `real_test.go` and pedestrian tests | Buggy, trike, bike, rover and truck acceleration, braking, gears, suspension, grip, lean/balance, impacts, enter/exit, recovery, headlamps and parking. |
| Injuries and ragdolls | `character/health.go`, `character/ragdoll*`, `game/injuries*`, `vehicle/impacts*` | Same injury thresholds, incapacitation/revival, momentum transfer, constrained limbs, settling and cleanup. |
| Rendering | `shading/`, `game/sky*`, `daylight.go`, `weather*`, `lamps*`, `effects*`, `cull*` | Same painted/toon look, character-only outlines, hair/glass coverage, interior light zones, sky, fog, shadows, weather and particles. |
| Camera and interactions | `game/camera*`, `game/use*`, `game/drive*`, `game/teleport*`, `character/seat*` | Same camera feel, pointer capture, prompt priority/reach, machines, seats, emotes, map teleport and vehicle teleport. |
| Menus and gameplay UI | `game/menu*`, `screens.go`, `paper*`, `ui.go`, `wardrobe*`, `contracts*`, `ledger.go` | Title/pause flows, parchment cursor and typography, wardrobe, first work order and delivery, journal/debt, map, keyboard/mouse navigation and HUD. |
| Audio | `game/sound.go`, `ambience.go`, `drivesound.go`, `cues*` | Same sounds, timing, pitch, looping, attenuation, pause behavior and browser activation. |
| Accounts | `identity/`, `internals/account/`, `game/identity*`, `web/spacetime/` | Cross-language signing vectors, encrypted backup interchange, storage paths/keys, offline management, Unicode names, connect/reconnect, latency and error handling. |
| Testing tools and crowds | `game/testing*`, `residents.go`, `tools/tour/`, `tools/shot/` | Existing test controls, crowd count and spawn batches, relocation and cleanup, capture views and benchmark behavior. |
| Delivery and tooling | `Makefile`, `cmd/`, `.github/workflows/ci.yml`, `tools/desktop/`, `tools/ota/`, `tools/ci/`, Railpack files | Desktop architecture matrix, browser serving/compression, public build config, installers, asset staging, release manifests and rollback. |

Retain current platform budgets. Desktop uses HighDPI, MSAA, 4096-pixel
shadows, full terrain budgets and cloth. Browser uses CSS-pixel resolution,
no MSAA, 1024-pixel shadows, lighter distant detail and skeletal clothing.
Both keep near-terrain collision accuracy. Preserve the existing Full/Low/Off
shadow control. Optimization cannot mean lowering these settings.

## Target layout

Use the root Cargo workspace with the client under `apps/client/`, alongside
the Go reference. Start with one client crate and modules corresponding to `game`, `world`,
`character`, `vehicle`, `shading`, `identity` and `assetref`. Use Bevy plugins
for engine-facing modules; keep data formats and identity logic independent
of rendering. Extract additional crates only when there is a real reuse or
compilation boundary, placing shared crates under `crates/`. Keep generated
artifacts under `build/`. See [workspace organization](workspace.md).

Port the Go implementation directly, function by function where practical.
Keep its constants, formulas, control flow, input consumption and observable
ordering; replace engine calls with the Bevy/Avian equivalent. Prefer matching
module and function names over a redesign. Use idiomatic Rust ownership, enums,
resources, components and plugins at the engine boundary, and document any
scheduling or physics difference that prevents a direct translation. Never
retune an existing threshold merely to make the port's tests pass. Organization
changes belong at the workspace/module boundary and must preserve behavior.

Use existing assets and manifests. Convert engine handles, entity identifiers
and math types internally; do not expose Bevy entity IDs as persistent or
network identifiers. Preserve units, axis conventions, model scale, transform
hierarchy, bone mapping and material semantics explicitly.

Keep `internals/server/module` as the existing Rust SpacetimeDB module.
Gameplay that is currently local stays local during this migration. Keep
the web networking adapter until equivalent Rust/WASM transport behavior is
verified; changing language is not a reason to redesign the account protocol.

Add separate Rust build/check/run targets and CI checks before changing any
default Go target. Keep Rust candidate releases separate from the production
OTA channel until cutover. Update release input detection when Rust becomes
a shipping input, and preserve rollback to the previous release.

## Scene composition with BSN

Use Bevy Scene Notation for reusable scene composition: world-piece
assemblies, character and vehicle hierarchies, menus and HUD elements.
Start with the `bsn!` and `bsn_list!` composition APIs demonstrated by
Bevy 0.20's official example. Verify any external `.bsn` loading or hot
reload workflow separately before depending on it.

Keep existing JSON world/layout manifests as the authoritative content inputs
during parity work. An adapter (`apps/client/src/world.rs`, over the formats
in `crates/world`) resolves their stable piece IDs, transforms, colliders,
ladders, lights and vehicle specifications into reusable BSN assemblies:
each placement is a root carrying the piece name, with its parts as
children, under a layout root that despawns the whole layout. Reuse GLBs and textures without reauthoring them. This avoids
maintaining two competing copies of Landfall's placement data.

Rust systems continue to own movement, physics stepping, gameplay rules,
procedural terrain, streaming and network synchronization. BSN describes
the entity composition those systems use. Runtime entity IDs remain internal;
scene composition must preserve the logical IDs used by assets and accounts.

The initial proof should compose a world prop, a character, a vehicle and
one menu panel. Verify hierarchy, local/world transforms, collision placement,
asset readiness, spawn/despawn cleanup and repeated instantiation on desktop
and browser. Compare the assembled output to the existing manifests and
reference scenes. Preserve the current world mesh merging/batching benefits;
declarative composition alone is not a performance improvement.

## Completion plan

The objective is the same playable Earth Two implemented in Rust/Bevy,
including the surrounding programs needed to build and ship it without Go.
Reuse the existing Rust ports after verification. Preserve the current Bevy,
Avian and Rust pins while establishing parity; dependency upgrades are
separate changes with the same regression gates. Preserve JSON manifests,
assets, stable identifiers, account formats and the existing Rust database.

Work in `silent-ibis` on `golang-rust`. First preserve the inherited changes
as a reviewable source checkpoint. Keep generated binaries, caches and local
editor settings out of migration commits. Each subsequent change should
complete one observable scenario and include its test/evidence and updated
status. Keep Go build/run/release defaults until the final cutover.

### Acceptance contract

“Exactly as today” means matching what the player sees, hears, controls and
can do, and keeping their existing data usable. It does not mean assuming
that Avian will produce bit-identical Jolt trajectories or that different
renderers will emit identical pixels.

- Exact: control bindings, UI text, menu flow, gameplay state transitions,
  interaction priority, inventory/content identifiers, account protocol
  bytes, file schemas, crypto parameters and lossless data interchange.
- Reference-measured: movement and vehicle response curves, collision
  timing, camera motion, animation timing, cloth/ragdoll trajectories and
  rendering. Set tolerances from repeated Go runs before tuning Rust.
  Preserve discrete outcomes at boundaries: clearance, vault success,
  injury thresholds and contract rewards cannot change within a tolerance.
- Perceptual: paired screenshots, clips and audio captures under identical
  conditions must retain the established art style, timing and feel.
  Automated comparisons support a human side-by-side playthrough.
- Platform and performance: retain existing desktop/browser capabilities
  and quality settings. Establish acceptable variation from repeated
  baseline runs; average FPS alone cannot conceal startup or frame spikes.

A subsystem progresses through **implemented → integrated → compared →
accepted**. Only accepted subsystems count toward completion. Compilation,
headless tests and a showcase screenshot alone cannot confer acceptance.
Unresolved differences remain defects; do not silently relax the contract.

### 1. Freeze and measure the Go reference

Run the Go tests through the prepared dependency workspace and distinguish
existing failures from migration regressions. Inventory every subsystem in
the source table above, including its tests, platform-specific paths and
configuration flags. Give each acceptance scenario a stable ID, reference
source/test, setup, inputs, expected outcomes, comparison method and evidence
paths. Maintain this matrix in repository documentation.

Capture the actual game on desktop and browser: title/new session, Landfall
arrival, traversal block, every vehicle, interiors, day/night/weather,
wardrobe, crowds, injuries/revival, contracts, maps, menus and accounts.
Use normal desktop cloth for its baseline: `buildConfigured(..., capture)`
changes cloth and window behavior and must not substitute for normal play.
Use disposable identities for account scenarios.

Add fixed-tick recording/replay and trace export to both clients where the
existing tour/shot/trailer tools are insufficient. Record consumed inputs,
seed, clock/weather, transforms, velocities, controller/animation states and
gameplay events. Represent entities by scenario IDs, never runtime ECS IDs.
Include low and high render rates, multiple fixed ticks per frame, pauses,
focus loss and scene reloads. Record asset hashes and build/environment
metadata with every capture.

Establish the native matrix (macOS arm64/x64, Linux x64, Windows x64) and
record the actual supported browser/device matrix by running the Go client.
Exercise Chromium, Firefox and WebKit candidates; record version, OS and
backend, and resolve observed support rather than assuming WASM compilation
proves it. Benchmark no crowd, the default 25 NPCs and existing stress cases.

**Exit:** a reproducible reference suite covers every source-inventory row,
with known Go failures, tolerances, platform coverage and baseline artifacts.

### 2. Close the engine and vehicle blockers

Fix the two known vehicle regressions without weakening assertions:
motorcycle cornering stability and the straight approach to a pedestrian.
Instrument wheel cast hits/normals, suspension loads, tyre impulses, body
inertia/centre of mass, steering and lean response; compare the same Go
runs. Make the real-vehicle tests report each vehicle independently so the
bike's first failure no longer hides buggy, trike, rover or truck failures.

Verify physics ordering, CCD, capsule/shape casts, parented colliders,
interpolation, pause/resume and teleports. Test fixed input consumption under
variable rendering cadence. Run the smoke and asset-loading scenarios in an
actual browser as well as on the native matrix. Load original and packed
GLBs, external packed resources, textures and the audio assets used today.

**Exit:** all existing Rust tests pass, every vehicle case executes, the
physics scenarios match their Go references, and native/browser runtime
foundation checks pass. Keep physics risks open until compared evidence exists.

### 3. Assemble the playable application

Add an application/game plugin that owns the lifecycle and ports
`game/game.go`'s setup and schedule. Keep the foundation viewer as a separate
diagnostic command. Use explicit states for loading, title, playing and
paused, matching the Go transitions and cleanup behavior. Define resource
ownership and teardown so repeated title/play transitions do not duplicate
players, subscriptions, audio, world entities or input handlers.

Connect the existing world, terrain, character, presentation, vehicle,
shading, sky and identity modules. Wire player spawn/starting outfit,
camera orbit/follow/collision, pointer capture, keyboard/mouse input,
interaction priority, enter/exit and seats. Connect stream centre, camera
culling, nearby ground coverage, light zones, night/headlamp state and
asset readiness/error paths. Preserve BSN composition from the existing
manifests rather than inventing new content layouts.

Audit the schedule against Go: input and interaction consumption before
character actions; traversal before locomotion; vehicle forces before
physics; impacts after physics writeback where appropriate; pose fitting,
seated hands/feet, clothing and bone attachments in the right order; camera
follow and world streaming after the states they read. Add integration tests
for these dependencies rather than relying on plugin insertion order.

**Exit:** on desktop and browser, a player can start a session, walk through
Landfall, use the traversal block, enter/drive/exit a buggy, pause/resume and
return to title and resume the same session without duplicate entities. This is the first complete playable
slice; unfinished UI/art features remain explicit entries in the matrix.

### 4. Finish movement, vehicles, injuries and residents

Compare all locomotion/traversal tests and recorded routes, including slope
and stair edges, crouch clearance, vault/mantle rejection, ladder endpoints,
wall kicks, slides, rolls, landing and collision while streaming. Complete
seating and ride poses, attachment transforms and controls when downed.

Finish every vehicle family: acceleration/braking/reverse/gears, steering,
suspension, low-speed balance, terrain transitions, blocked exits, recovery,
parking, tyres, lamps and collision with people/vehicles/world. Test both
crowd LOD and full-rate pedestrians and inspect threshold boundaries.

Port the remaining hybrid ragdoll behavior, injuries, revival, impacts and
cleanup from `character/ragdoll*` and `game/injuries*`. Port resident behavior,
crowd spawn batches, relocation, test controls and despawning. Reproduce the
current architecture before attempting a physics redesign.

**Exit:** recorded gameplay scenarios and their boundary outcomes agree for
all characters and vehicles, including repeated spawn/despawn and collisions.

### 5. Match world, visuals, animation and sound

Verify Landfall placements and procedural terrain/scatter determinism,
streaming collision readiness, map coordinates, merge/batching, draw distance,
visibility and object cleanup. Confirm transitions across streamed boundaries
at walking and vehicle speeds and after teleporting.

Finish shader/material integration and compare painted lighting, skin,
character-only outlines, hair/glass/transparency, textures, shadow coverage,
interior zones, fog, sky, celestial bodies, weather and particles. Match color
space, tone mapping and anti-aliasing deliberately. Complete animation blend
and clip timing, outfits, hidden body meshes, footwear, foot placement,
attachments and desktop cloth; preserve browser skeletal clothing.

Port `sound.go`, `ambience.go`, `drivesound.go` and cues. Match clips, gain,
pitch, loops, attenuation, surfaces, footsteps, engines, impacts, ambience,
pause/resume and teardown. Add and validate the needed audio backend/features;
test browser audio activation through the same user gestures as Go.

Preserve desktop HighDPI/MSAA/full terrain/4096 shadows and browser CSS-pixel
resolution/no MSAA/lighter distant detail/1024 shadows, including Full/Low/Off
shadow selection. Use paired normal-play captures, including cloth and audio,
rather than accepting showcase scenes as final visual evidence.

**Exit:** the visual/audio scenario matrix passes at matching settings, with
no missing world collision, content, effects or presentation behavior.

### 6. Complete menus, interactions, gameplay and accounts

Port the title/pause/settings flows, paper styling/cursor, fonts, text layout,
focus/navigation, wardrobe, HUD, maps/teleports, journal/debt and contract
screens. Match prompts, priority, reach, benches/bunks/machines, emotes,
punch/pickup behavior, the existing work order/delivery loop and rewards.
Audit all `game/*_test.go` cases for a Rust equivalent or a documented reason
that they are superseded by another acceptance test.

Connect the identity panel to the actual menu. Verify existing native files
and browser storage, create/unlock/import/export/restore, Unicode names,
offline operation, wrong-password and malformed-file errors, and cross-client
signing/encrypted-backup fixtures in both directions. Exercise actual
connect/reconnect, latency and failure handling against a disposable database
using the existing schema and authorization protocol.

Fix browser packaging to build/stage/load the existing `EarthTwoAccount`
adapter before WASM starts: `tools/rust/web.py` and the current browser shell
do not yet include it. Test imports/downloads/storage through a real browser,
not only the adapter's emulated-storage harness. Keep secrets out of captures.

**Exit:** the full current player journey runs in Rust on native and browser,
including account reuse and the first contract, with matching UI and errors.

### 7. Remove Go from tooling and delivery

Replace Go programs in independently reviewable groups: desktop launcher and
server companion, static/browser server, landing server, account CLI;
asset packing/catalog/fingerprint tools and build configuration; web account
injection/compression; screenshot/tour/trailer/identity preview tools; and
GLB, painting and bind-pose utilities still needed for authoring/builds.
Retire `tools/deps` once no Go/native dependency preparation needs it rather
than reproducing obsolete setup. Retain useful existing Python/TypeScript
utilities and the already-Rust database module.

Compare CLI flags, exit codes, environment variables, paths, output formats,
asset graph/fingerprints and HTTP behavior against the existing tests.
Preserve compressed serving, caching, content types and public build config.
Update Make targets, CI change detection, candidate packaging, architecture
matrix, installers, manifests, checksums and Railpack entrypoints together.

Build native and browser release artifacts in a clean environment without Go
installed. Keep Rust releases on the candidate channel while validating fresh
install, update from the current Go release, data retention, failed-update
recovery and rollback to Go. Preserve the production OTA contract.

**Exit:** supported release/authoring workflows have accepted replacements,
release production requires no Go, and install/update/rollback tests pass on
the supported matrix.

### 8. Run full parity, performance and release acceptance

Run the complete Go/Rust scenario suite against identical assets, inputs,
quality, weather/time, wardrobe and population. Compare state/event traces,
paired screenshots/clips/audio and a human end-to-end playthrough. Include
long sessions, repeated scene/account transitions, focus/resize/fullscreen,
network interruptions and rapid streaming/teleports. All supported browser
and desktop combinations need runtime evidence, not just CI compilation.

Measure frame-time mean/p95/p99, CPU/physics/GPU time where supported, memory,
startup, stalls, download and installed size. Profile remaining bottlenecks
and optimize within the established quality and behavior contract. Repeat
matched release measurements after each substantive optimization. Preserve
raw results and report variability; unmeasured platforms remain unverified.

**Exit:** every acceptance-matrix row is accepted, no unexplained parity
defects remain, performance meets the reference-derived budget and the
candidate passes packaging/update/rollback acceptance. Gather user acceptance
of the concrete side-by-side candidate before changing the default release.

### 9. Cut over and retire the Go implementation

Switch normal run/build/package/browser and release automation to Rust only
after acceptance. Archive the exact Go reference and its evidence, keep a
known-good rollback release, and verify the installed Rust release still
opens existing accounts and assets. Then remove superseded Go implementation,
obsolete native dependencies and duplicate scripts while retaining reusable
fixtures, scenario definitions and migration history. Update developer and
player-facing instructions to the shipped behavior.

**Exit:** the normal game, development and release paths use Rust/Bevy;
current users retain their data and the accepted gameplay/presentation;
rollback is verified; no production build/runtime path depends on Go.

### Immediate execution order

1. Preserve the source checkpoint and record the Go baseline/test inventory.
2. Add trace diagnostics and independent per-vehicle reporting; fix and
   compare motorcycle stability and pedestrian approach without loosening tests.
3. Close browser physics/asset runtime checks and stage the account adapter.
4. Assemble the real application lifecycle and first playable Landfall slice.
5. Expand that slice through milestones 4–8, then perform the accepted cutover.

Steps 1 and 2 provide the evidence needed to estimate the remaining work.
Do not assign a completion percentage from module/file count or commit to a
calendar date before the physics, rendering and browser risks are measured.

## Baseline and performance procedure

Use the current commands from the repository root:

```sh
make test
make assets
EARTH_TWO_PACK_CHECK="$PWD/build/assets" go test ./tools/assetpack -run TestPackedRepository
```

After `make deps`, use the prepared native dependency workspace for captures
and CPU benchmarks:

```sh
GOWORK="$PWD/build/deps/native.work" EARTH_TWO_TOUR_STATS=1 EARTH_TWO_HOUR=11 go run ./tools/tour
GOWORK="$PWD/build/deps/native.work" go test ./game -run '^$' -bench 'BenchmarkMinimapSelection|BenchmarkTerrainTileMask' -benchmem
```

For browser measurements run `make serve`, then visit
`http://localhost:8080/?benchmark=1`. Keep timer-driven headless throughput
(`timers=1`) separate from display-paced browser results. Existing browser
CPU/render submission timings are not direct GPU execution measurements.

Record commit, dependency versions, hardware, OS, browser, backend, resolution,
quality settings, scene, crowd population, time/weather, warmup and sample
duration. Run matched Go and Rust release builds multiple times on the same
device. Preserve raw results and report median plus variability across runs.

Measure mean/p95/p99 frame time, simulation/physics time, CPU submission time,
GPU time where supported, peak memory, cold/warm startup, compressed download
size and installed size. The existing tools provide only part of this data;
add missing metrics explicitly. Use uncapped diagnostic runs to expose CPU/GPU
headroom, but also test normal VSync/display-paced play. Record loading stalls
and terrain boundary spikes rather than hiding them in average FPS.

Set the allowed regression band from baseline run-to-run variation before
comparison. A faster average does not excuse new stutters, changed handling,
missing effects or reduced detail. No performance gains are claimed until
the matched measurements exist.
