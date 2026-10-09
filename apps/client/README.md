# Earth Two Rust client

The migration preview uses Bevy 0.20.0, Avian and BSN. The default
executable opens Landfall with a player, walking/traversal, vehicle controls,
a following collision camera, and loading/title/play/pause states. Returning
to the main menu preserves the current session, as in Go.

This is a playable integration checkpoint. Hair and garments follow the
body skeleton; bike/trike riding, static-wall pose fitting, articulated
knockdowns and R recovery are connected.
Remaining animation/foot placement, lighting and complete visual parity remain unfinished, as do sound, full
menus/accounts, residents and other gameplay flows. Go remains the
release default. See the [migration checkpoint](../../docs/rust-migration.md#step-4-injury-review-checkpoint-2026-10-09)
for evidence and remaining work.

Run from the repository root:

```sh
make rust-run       # playable Landfall preview
cargo run --locked -p earth-two-client -- --at=buggy
cargo run --locked -p earth-two-client -- --at=bike
cargo run --locked -p earth-two-client -- --at=injury
cargo run --locked -p earth-two-client -- --at=fatal
cargo run --locked -p earth-two-client -- --at=traversal
cargo run --locked -p earth-two-client -- --foundation
make rust-smoke     # finite headless gravity/contact check
make rust-test      # whole workspace, without a window
make rust-check     # native and browser compile checks
cargo run --locked -p earth-two-client --example game_smoke
cargo run --locked -p earth-two-client --example game_smoke -- --bike
cargo run --locked -p earth-two-client --example injury_smoke
cargo run --locked -p earth-two-client --example injury_smoke -- --fatal
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Click Play or press Enter. WASD/arrows move, Shift runs, Space jumps/traverses,
C crouches, Ctrl slides and R rolls. E enters/exits a nearby vehicle; driving
uses WASD/arrows, Space handbrake, H headlamps and R recovery. Esc pauses or
resumes; arrows and Enter select menu choices. Desktop mouse movement orbits;
in a browser focus the canvas first and drag with either mouse button.

`--at=buggy` and `--at=bike` place the player beside the authored vehicles
nearest Landfall arrival. Press E to mount, then inspect the hands and feet
while driving and after dismounting.
`--at=traversal` adds the authored Hull test block above the terrain for
isolated review. `--foundation` preserves the earlier asset/physics diagnostic.
`--at=injury` and `--at=fatal` trigger a diagnostic knockdown two seconds
after Play, using 25 and 50 km/h impact states. They directly invoke the
vehicle knockdown path; they do not stage a collision. Inspect the limbs,
hair and clothes, pause/resume, then press R to return to the Pads. The
`injury_smoke` example asserts this flow with the native renderer.
The `game_smoke` example uses synthetic game inputs in the native renderer
and asserts play/drive/exit/walk/pause/title behavior before saving a capture.

The manifests load through AssetServer on native and browser. Their BSN
assemblies retain the piece names, model paths, placement transforms, and
child colliders, ladders, lamps and wheels. Assets remain shared with Go;
no converted copies are checked in. Set `EARTH_TWO_ASSET_ROOT` to inspect a
native asset pack. PNG and JPEG decoding are enabled for existing textures.
For an automatic title capture use
`EARTH_TWO_CAPTURE=build/rust/title.png make rust-run`; it waits for content
and a render warmup, captures a frame and exits.

Local evidence includes 270 passing headless tests, 90 client library tests
with rendering/pose regressions, native macOS arm64/Metal graphical smoke, and a packed
asset Chromium/SwiftShader WebGL2 gameplay check. CI also compiles/tests the
Linux, Windows and both macOS architecture targets. Other runtime platforms,
performance and exact Go parity remain unverified.

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

The client uses the WebGL2 feature included by Bevy's platform profile. Its
canvas is `#earth-two`; browser assets are relative to `assets/` alongside
the generated JavaScript/WASM. `web/index.html` is the static browser shell.

Run `make rust-web` to build WASM and stage the shell and existing packed
assets in `build/rust-web/`. This requires Python 3.11+ and the `wasm-bindgen`
CLI matching `Cargo.lock`; the script prints the exact install command when
it is missing. Serve with
`python3 -m http.server 8080 --directory build/rust-web`.
Use `/?at=buggy`, `/?at=bike`, `/?at=injury`, `/?at=fatal`, `/?at=traversal`, or `/?foundation` for review routes.
Browser runtime validation is separate from `make rust-check`, which only
compiles for WASM.

See [workspace organization](../../docs/workspace.md) and the
[migration plan](../../docs/rust-migration.md).
