# Rust migration with Bevy and Avian

Earth Two will migrate from Go, illusion, raylib, Ark ECS and Jolt to Rust,
Bevy **0.20.0** and Avian 3D, with BSN for scene composition. The migration must preserve the existing game:
its appearance, controls, movement, vehicle handling, content, account data,
desktop and browser support, and release workflows. Performance improvements
must be measured at equivalent quality and workload.

Status: substantial subsystem ports exist in `apps/client/`,
`crates/world/` and `crates/identity/`, but the executable still opens the
foundation viewer. The last validation found 238 passing headless tests and
two failing vehicle scenarios; native/WASM checks and Clippy passed.
A working module or green unit test is not evidence of integrated game parity.
The completion plan below is the execution order from this checkpoint.

The reference for “the game today” is the Go implementation at
`c561bd69e06500aa3c79e9bb0d0d5e974694e7c7`, currently both this worktree's
HEAD and local main. Capture its asset hashes, dependency locks and build
configuration along with the source. Features described only in the world
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
return to title with clean restart. This is the first complete playable
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
