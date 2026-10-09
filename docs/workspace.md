# Workspace organization

The root Cargo workspace contains the Rust migration client. The playable Go
reference and existing SpacetimeDB module retain their paths and release
commands until the Rust client passes the migration gates.

| Location | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | Rust client membership, reproducible dependencies and toolchain. |
| `.cargo/config.toml` | Shared Rust build settings; artifacts go to ignored `build/rust/`. |
| `apps/client/` | Rust desktop/browser executable, game modules, integration tests and browser shell. |
| `crates/identity/` | Rendering-independent account protocol, crypto, compatible files/backups and native/browser sessions, with cross-language fixtures. |
| `crates/world/` | World manifest formats, piece-frame placement math and the deterministic terrain, independent of rendering; its `fixtures/` hold Go-generated reference data. |
| `assets/` | Shared source assets; both clients consume the same content. |
| `docs/` | Design, contracts, migration plan and workspace guidance. |
| `tools/` | Asset generation, developer commands, packaging and release tools. Rust-specific scripts belong in `tools/rust/`. |
| `internals/server/module/` | Existing independent Rust database workspace and lockfile. Excluded from the client workspace. |
| `.github/workflows/rust.yml` | Rust candidate checks. No release or OTA publishing. |
| `.github/workflows/ci.yml` | Existing Go builds and release workflow. |
| `build/` | Generated assets, binaries, baseline captures, temporary source checkouts and caches. Never source code. |
| `game/`, `character/`, `vehicle/`, `world/`, `shading/`, `identity/`, `assetref/`, `cmd/`, `internals/account/`, `web/` | Existing Go game and supporting services; retained as the parity reference. |

Add reusable Rust libraries under `crates/` when a second consumer or a clear
build boundary needs them; `crates/world` exists because tools and the
server-side checks need the formats without Bevy. Keep client-only modules inside `apps/client/src/`
until then. If a local third-party patch is required, put its source under
`vendor/` with upstream revision, license and a documented diff; exclude its
upstream workspace from ours. These directories are created when used.

Avian currently uses an immutable upstream compatibility revision and pinned
companion repositories, so no vendored copy is needed. The lockfile records
the exact sources. Temporary investigation checkouts under `build/migration/`
are not dependencies and can be removed without changing the source build.

Run client Cargo commands from the repository root. Existing `make run`,
`make build`, `make web` and release commands continue to target Go. Use
`make rust-run`, `make rust-smoke`, `make rust-test` and `make rust-check`
for the migration client. The database module continues to use its own
`--manifest-path` and `--locked` build in `make server-module`.

Move legacy source only when its replacement is accepted, updating its
imports, asset roots, CI input rules and release tooling together. Directory
cleanup must not discard behavioral tests or make the reference unbuildable.

See the [client instructions](../apps/client/README.md) and
[migration milestones](rust-migration.md).
