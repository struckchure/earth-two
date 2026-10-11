# Native mobile support for iOS and Android

Date: 2026-10-11

Status: Draft implementation plan; feasibility and estimates require device validation.

## Goal

Ship Earth Two as native Android and iOS applications while retaining the shared
Go gameplay code, world, assets, identity format and backend architecture.
The first release should make the existing playable game comfortable on phones,
with reliable installation, touch controls, suspension/resume and account access.

Start with landscape play and ARM64 physical devices. Select minimum operating
system versions and supported device tiers after profiling. Tablet layouts and
external controllers should fit the architecture, but are not initial release gates.
This plan does not add shared multiplayer simulation, cloud saves, monetization
or new gameplay systems. Mobile clients inherit those features as they become
available in the shared game.

## Existing foundations and gaps

| Area | Current foundation | Mobile work |
| --- | --- | --- |
| Game and engine | Go, illusion, raylib-go and Jolt; desktop and browser entry points | Mobile app hosts, build configuration and engine lifecycle integration |
| Input | Browser touch controls in `tools/webaccount/touch.js`, consumed by `game/touch_js.go` | Native multitouch adapter and controls; `game/touch_native.go` currently provides stubs |
| Graphics | Desktop and browser budgets in `game/budget_gl.go` and `game/budget_js.go` | Dedicated mobile settings and mobile-compatible shader selection |
| Physics | illusion compiles Jolt through cgo | Android/iOS compiler, linker, threading and runtime validation |
| Assets | Packed release assets, loaded using platform-specific paths | App bundle access, writable cache paths and measured memory/install budgets |
| Identity | Shared encrypted backup format and account logic | App-private persistence, platform key protection, keyboard and document picker integration |
| Networking | Native Go SpacetimeDB client and browser adapter | Native client compatibility, reconnect behavior and network-change testing |
| Distribution | Desktop installers, browser builds and OTA publishing tooling | Mobile signing, beta distribution, store packaging and a separate mobile update policy |

The current `!js` build constraints often mean desktop rather than all native
platforms. Audit these assumptions throughout the game and illusion. Examples
include desktop window creation, home-directory storage, desktop GLSL, cursor
capture and rendering budgets. Do not let mobile silently inherit desktop defaults.

The pinned raylib-go dependency includes Android integration using OpenGL ES 2.
Its macOS integration is not an iOS implementation. Android therefore has the
clearer initial path; iOS needs a rendering and app-host prototype immediately.
Jolt itself supports both platforms, but that does not establish compatibility
of illusion's current bindings and build flags.

## Architecture

Keep gameplay, simulation, world generation, character and vehicle systems,
contracts, identity cryptography and account protocol in shared Go packages.
Introduce small platform interfaces at the actual integration boundaries:

- App startup, frame scheduling, pause/resume and shutdown.
- Touch/pointer input, text entry, clipboard and screen safe areas.
- Read-only asset access and writable save/cache locations.
- Protected identity storage and document import/export.
- Audio interruption handling and connectivity notifications.

Use Android and iOS app hosts to implement those boundaries. Keep their language
and bridge choices provisional until the first engine prototype establishes how
Go, raylib and the operating system share the main thread and graphics context.
Do not assume `gomobile` alone can package the existing window loop unchanged.

Add explicit desktop, browser and mobile platform selection. Keep graphics
capabilities separate from operating-system names: an OpenGL ES backend must
receive compatible shaders regardless of whether it runs in a browser or an app.
Keep dependency changes in a reproducible pinned fork or the existing dependency
preparation workflow; do not rely on manual edits to the Go module cache.

## Phase 1: Prove the engine on both platforms

Deliver Android's first playable prototype while investigating iOS in the same
phase. Resolve iOS feasibility before committing to a full mobile rollout.

### Android

- Establish an ARM64 Android build using the SDK/NDK and raylib-go's Android
  integration. Adapt illusion's Jolt C++ linking for the Android toolchain.
- Create a minimal app host with bundled assets and a debuggable installable build.
- Render a representative Earth Two scene using compatible shaders, then enable
  animated characters, Jolt collision, vehicle simulation, audio and basic touch.
- Exercise app background/resume, surface recreation and process restart.

### iOS

- Create an Xcode host and prove Go/C/C++ linking and main-thread ownership on a
  physical iPhone.
- Evaluate the community raylib iOS port and its graphics backend against our
  pinned raylib-go API, shaders, skinning, render targets, shadows and audio.
- Record the selected rendering route, required patches, licensing, dependency
  versions and ongoing maintenance burden. A community port is a candidate,
  not an assumed drop-in dependency.
- Run the same representative scene and lifecycle checks as Android.
- If the existing renderer cannot be integrated reliably, document the scope of
  a backend replacement and re-estimate before proceeding with that larger change.

### Acceptance criteria

- Reproducible builds install and run on a physical Android phone and iPhone.
- Each device renders an animated character in a representative game scene with
  working collision, vehicle physics, audio and touch input.
- Background/resume does not crash, leave stuck input, lose audio permanently or
  advance simulation by the entire suspended duration.
- Capture a sustained 20–30 minute run: frame-time distribution, peak memory,
  thermal behavior, loading time and installed size, with device/OS/settings recorded.
- Produce a feasibility decision and revised estimate, including all required
  illusion/raylib changes. A blank-window demo alone does not pass this gate.

## Phase 2: Shared input and mobile UI

- Define shared gameplay actions and analog values for movement, look, aiming,
  firing, interacting, traversal and driving. Preserve keyboard/mouse behavior.
- Adapt browser touch input to the same action model and add native multitouch
  input with stable touch ownership, cancellation and independent movement/look.
- Implement a movement stick, drag-to-look, contextual interaction, combat and
  vehicle controls. Support simultaneous movement, camera control and actions.
- Make UI consume touches before gameplay; opening menus or losing focus clears
  held actions. Account for display scaling and safe areas.
- Rework menu, inventory, map, contract and identity screens for readable text
  and finger-sized targets. Integrate native text entry for Unicode, passphrases
  and editable fields without recording sensitive contents in diagnostics.
- Provide sensitivity and control-size settings; retain them between launches.

Acceptance: a player can start the game, manage identity, explore, traverse,
drive, fight, use inventory and complete the currently available contract flow
without a keyboard or mouse. Touches on UI never accidentally fire or move the
character. Browser and desktop input continue to behave correctly.

## Phase 3: Lifecycle, persistence and networking

- Separate initialization, frame execution, suspension and resource restoration
  where required by the app hosts. Reset elapsed-time accumulation on resume.
- Handle audio interruptions, screen locks, app switching, Android Back behavior,
  graphics-surface loss and operating-system termination.
- Resolve bundled assets independently of the working directory. Put persistent
  data and disposable caches in the appropriate app directories.
- Retain the existing encrypted identity backup format. Design platform protection
  using iOS Keychain and Android Keystore-backed encryption where appropriate;
  do not assume the existing signing key can be imported directly into hardware.
- Add native document pickers/share flows for backup import/export, atomic writes
  and recovery from interrupted imports. Explain backup needs before uninstall or
  device loss can leave a player without their identity.
- Validate the native account client on both platforms. Reconnect after backgrounding
  and Wi-Fi/cellular transitions, and surface offline/error states without blocking
  local account management. Do not blindly replay non-idempotent operations.

Acceptance: identity persists across normal restarts and upgrades; backups move
between desktop, browser and mobile without changing account identity. Interrupted
writes do not destroy the last valid backup. Suspension and network loss recover
cleanly, with explicit account connection state.

Gameplay persistence and shared multiplayer remain separate work: the current
README describes movement and the first contract as local. This phase does not
promise cross-device progression or simulate an always-running background game.

## Phase 4: Performance and asset budgets

- Start with a mobile profile near the browser budget: smaller shadows, reduced
  distant terrain/scatter, no default MSAA and skeletal clothing rather than cloth
  simulation. Preserve collision accuracy and gameplay rules.
- Target sustained 30 FPS on the selected minimum device tier; offer 60 FPS on
  capable devices after measurement. Keep fixed-step physics independent of render
  rate and prevent unbounded catch-up after stalls.
- Profile CPU, GPU and memory separately in dense scenes, driving, combat and
  terrain transitions. Include sustained runs that expose thermal throttling.
- Control internal rendering resolution rather than automatically rendering at
  every physical display pixel. Add quality levels based on measured bottlenecks.
- Measure packed assets, decoded textures, audio, loading spikes and graphics memory.
  Set numerical budgets from the device prototype before declaring support tiers.
- Bundle enough content for the first playable flow. Introduce optional content
  downloads only if size/loading evidence justifies them; require integrity checks,
  resumable transfer and safe handling of low storage if implemented.

Acceptance: the agreed device matrix meets the recorded frame-time, memory,
startup and thermal budgets in representative sustained play. Report p95 frame
times and stalls alongside average FPS. No claim of low-end support is made until
it passes these checks.

## Phase 5: Build, distribution and release

- Add documented build targets, provisionally `make android` and `make ios`, with
  pinned Go, SDK/NDK, Xcode and engine dependencies where practical.
- Produce Android debug APKs and signed release app bundles, plus iOS archives
  suitable for TestFlight/App Store distribution. Keep signing secrets outside
  source and client artifacts.
- Extend CI with mobile compilation and packaging, while retaining desktop/web
  checks. Archive symbols needed to diagnose Go and native crashes.
- Use store-distributed executable updates for the initial mobile release. Audit
  existing OTA integration so desktop executable update behavior cannot run on
  mobile. Treat any downloadable assets as a separate versioned content mechanism.
- Validate current store and platform requirements at release time, including
  target SDKs, native library compatibility, privacy declarations, age ratings,
  account-related flows and signing/provisioning requirements.
- Run internal Android testing and TestFlight before public distribution. Add
  crash diagnostics that exclude keys, passphrases and account secrets.
- Document the release process, supported devices, troubleshooting and dependency
  maintenance ownership.

Acceptance: clean CI builds produce installable signed beta artifacts; upgrades
retain identity/settings; clean installs and low-storage failures behave correctly;
release metadata and diagnostics are ready for both stores.

## Verification matrix

| Coverage | Required scenarios |
| --- | --- |
| Devices | Minimum supported Android tier, a higher-tier Android with a different GPU family, minimum supported iPhone, a current iPhone |
| Gameplay | Traversal, vehicles, combat/aiming, inventory, contracts, dense world areas and terrain streaming |
| Input | Multiple fingers, cancelled touches, menu transitions, keyboard entry, display scaling and safe areas |
| Lifecycle | Repeated background/resume, screen lock, audio interruption, process kill and surface recreation |
| Accounts | Create/unlock, connect, backup export/import, failed import, offline state and network transition |
| Installation | Fresh install, app upgrade, retained identity/settings, low storage and missing/corrupt optional content |
| Regression | Existing desktop/web tests and representative smoke runs, including browser touch controls |

Add focused automated tests for action merging/cancellation, lifecycle time reset,
path selection and persistence failures. Use physical-device integration checks
for graphics, audio, thermals and platform UI; emulator success is insufficient.

## Sequencing and planning allowance

1. Prove the native engine and representative scene on both platforms.
2. Confirm the iOS backend, minimum devices and measurable performance budgets.
3. Implement shared input and platform services, then complete playable mobile UI.
4. Tune sustained performance and validate account/lifecycle reliability.
5. Produce signed beta builds, complete device coverage and prepare store release.

Initial allowance for one experienced engineer working full time: **2–4 weeks for
feasibility, followed by 6–12+ weeks for a mobile beta**, assuming a viable iOS
renderer path. These are estimates, not commitments. Renderer replacement,
significant asset rework or broader device support requires re-estimation. Store
review and external account provisioning are outside the engineering allowance.

The critical decision is whether iOS can reuse the current rendering stack at an
acceptable maintenance cost. Resolve it early even if Android reaches playable
status first. Prefer preserving the shared game over starting an engine migration
before that evidence exists.

## References

- [Game overview and platform behavior](../README.md)
- [Pinned Go dependencies](../go.mod)
- [Game startup](../game/game.go)
- [Native rendering budget](../game/budget_gl.go)
- [Browser rendering budget](../game/budget_js.go)
- [Browser touch adapter](../game/touch_js.go)
- [Native touch adapter](../game/touch_native.go)
- [Native identity storage](../game/identity_storage_native.go)
- [Asset path resolution](../game/assets.go)
- [raylib-go and Android example](https://github.com/gen2brain/raylib-go)
- [Community raylib iOS port to evaluate](https://github.com/ghera/raylib-iOS)
- [Jolt supported platforms](https://github.com/jrouwe/JoltPhysics#supported-platforms)

Repository observations and upstream references were reviewed on 2026-10-11.
No native mobile build or device benchmark has been completed as part of this plan.
