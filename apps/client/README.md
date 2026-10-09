# Earth Two Rust client

This is the first migration foundation, using Bevy 0.20.0, Avian and BSN.
It places the Hull test block from `assets/world/world.json` and
`assets/world/hull_block.json`, the same manifests the Go client uses, and
shows the existing character body and buggy chassis beside a falling physics
probe. The shared BSN floor, probe and block also run in headless tests. It
is not yet the playable game. Movement/traversal, vehicle simulation,
character presentation, Landfall streaming, sky/weather, painted shaders and
identity/accounts now have Rust modules and regression tests. The default
viewer still runs the foundation scene; these modules are not yet assembled
into the complete gameplay, menu and audio flow. Rendering examples live in
`examples/`; module READMEs describe their integration hooks.

The 2026-10-09 continuation check passes native and WASM compilation and
workspace Clippy. The headless suite has 238 passing tests and two failing
vehicle scenarios (motorcycle cornering stability and pedestrian approach).
See the [continuation checkpoint](../../docs/rust-migration.md#continuation-checkpoint-2026-10-09)
for commands, evidence and the next work.

The manifests load as assets (`world::KitAsset`, `world::LayoutAsset`) so
the browser fetches them too. Spawning a `world::SpawnLayout` composes each
placement into a BSN assembly: a root with the piece name, model path and
placement transform, and its colliders, ladders and lamps as children. Avian
attaches the child colliders to the piece's static body; the viewer loads
models onto `PieceModel` entities and lights onto `Lamp` entities. The lamp
intensity scale is a stand-in until the shading port.

Run from the repository root:

```sh
make rust-run       # development viewer; Esc pauses physics, Space resumes
make rust-smoke     # finite headless gravity/contact check; nonzero on failure
make rust-test      # BSN/physics/world tests without a window, whole workspace
make rust-check     # native and browser compile checks
cargo fmt --all --check
cargo clippy --locked -p earth-two-client --all-targets -- -D warnings
```

The native viewer loads `assets/` relative to the working directory. Set
`EARTH_TWO_ASSET_ROOT` to an absolute directory to inspect a staged asset pack.
The source assets are shared; no asset copies or conversions are checked in.
The foundation uses default Bevy materials to check loading, and does not
claim visual or performance parity with the game. In particular, importing
the character GLB alone does not reproduce the game's skin/material assembly.
PNG and JPEG decoding are enabled for the existing model textures.

For a repeatable native capture, run
`EARTH_TWO_CAPTURE=build/rust/foundation.png make rust-run`. The viewer waits
for all three imported scenes and a short render warmup, saves a frame, then
exits. This captures the Rust integration scene, not the Go parity baseline.

The initial foundation was verified on macOS arm64 with Metal: all three
scenes loaded and a frame was captured. The original three headless physics tests,
Clippy and WASM compilation passed at that foundation checkpoint. Other desktop platforms have CI checks
configured; browser execution and full game parity remain unverified.

## Dependencies

Bevy is pinned to `=0.20.0`. Avian is pinned to upstream PR
[1070](https://github.com/avianphysics/avian/pull/1070), revision
`34c9b008d2f40638a953d9218db48d54f661365d`, which updates it for Bevy 0.20
and Parry 0.31. Its companion revisions are:

| Crate | Repository owner | Revision |
| --- | --- | --- |
| `bevy_heavy` | `andriyDev` | `a692ccad7cbfc7e62ebd6c54aff49715eab8c0b0` |
| `bevy_transform_interpolation` | `andriyDev` | `4b9ff214bd782e5003da602025c77210abe27545` |
| `glam_matrix_extras` | `andriyDev` | `2a70128a003b82b906897353da3aa05863e42827` |

The upstream requirements allow the final Bevy 0.20.0 release; the root
lockfile resolves that release. No local changes to Avian are currently
needed. Replace these development revisions with a compatible stable release
when available, after the same native/browser and physics tests pass.

Physics runs at 60 Hz. Avian's default FixedPostUpdate schedule is retained;
rendering has its own cadence. These tests establish integration correctness,
not equivalence to Jolt's character or vehicle controllers.

## Browser

The viewer uses the WebGL2 feature included by Bevy's platform profile. Its
canvas is `#earth-two`; browser assets are relative to `assets/` alongside
the generated JavaScript/WASM. `web/index.html` is the static browser shell.

Run `make rust-web` to build WASM and stage the shell and existing packed
assets in `build/rust-web/`. This requires Python 3.11+ and the `wasm-bindgen`
CLI matching `Cargo.lock`; the script prints the exact install command when
it is missing. Serve with
`python3 -m http.server 8080 --directory build/rust-web`.
Browser runtime validation is separate from `make rust-check`, which only
compiles for WASM.

See [workspace organization](../../docs/workspace.md) and the
[migration plan](../../docs/rust-migration.md).
