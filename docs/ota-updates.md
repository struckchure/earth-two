# OTA update integration plan

Integrate OTA updates for browser and desktop together, with most implementation
work focused on desktop. Browser players already receive a deployed build on
refresh through the existing cache revalidation behavior. Keep that flow simple;
add release consistency and rollback without introducing an offline cache system.

Desktop players should be able to download a new release from the game and apply
it on relaunch. Ship code and assets as a compatible set, verify downloads before
execution, and retain the previous working release for recovery. This plan covers
both platforms; it proposes implementation work rather than changing runtime
behavior today.

Release packaging and S3 publishing are implemented in
[`tools/ota`](../tools/ota/README.md), with an S3 and CloudFront storage template
and a gated CI publishing job. Update metadata uses the shared SpacetimeDB
database through [the server integration contract](ota-server-contract.md).
Desktop installation and restart recovery remain subsequent phases.

## Current architecture

| Area | Current behavior | Integration consequence |
| --- | --- | --- |
| Game | Go, raylib, illusion and Jolt; [`game.Run`](../game/game.go) starts the app | Native code updates require a new process; browser code updates require a new page instance |
| Browser build | [`make web`](../Makefile) delegates to the pinned illusion build script | A release includes `game.wasm`, `wasm_exec.js`, `fs.js`, raylib JS/Wasm/data, Jolt JS/Wasm, and its HTML |
| Browser files | [`cmd/web/files.go`](../cmd/web/files.go) serves one build directory with `no-cache` and gzip sidecars | Revalidation does not ensure that concurrent requests receive the same release during deployment |
| Content | [`world/world.go`](../world/world.go) and [`character/outfit.go`](../character/outfit.go) load JSON and models at startup | World layouts, colliders, wardrobe and models must be validated as one compatible set |
| Asset paths | Desktop [`assetRoot`](../game/assets.go) prefers working-directory assets; [browser assets](../game/assets_js.go) use `assets` | Desktop release selection needs an explicit asset root; browser content must populate the shared filesystem before Go starts |
| Embedded content | Fonts in [`game/ui.go`](../game/ui.go) and shader strings in `shading/` compile into code | Updating these currently requires a code release |
| Progress | [`game/contracts.go`](../game/contracts.go) keeps contract state for the current run | An update restart loses current progress; returning to the title screen does not make a restart safe |
| Delivery | [CI](../.github/workflows/ci.yml) uploads artifacts; [`railpack.json`](../railpack.json) builds and serves the browser game | Artifact publishing, channel promotion and rollback still need a release pipeline |

The pinned illusion version preloads `assets/` into `raylib.data`, initializes
raylib and Jolt, installs a Go filesystem bridge over raylib's filesystem, then
starts `game.wasm`. Preserve that order. Emscripten supports separate preload
packages, which provides a later path to separating content from runtime builds.
[Emscripten packaging documentation](https://emscripten.org/docs/porting/files/packaging_files.html).

## Shared release contract

Build each release once and assign an immutable release ID. Publish its manifest
and artifacts, verify them at the serving origin, then promote a small channel
pointer. Use preview, beta and stable channels. Promotion and rollback select
existing artifacts without rebuilding them.

```text
CI builds and validates browser and desktop artifacts
                         |
                         v
            Immutable release files and manifest
                         |
                         v
               Promote channel pointer
                  /              \
                 v                v
      Browser page refresh    Desktop launcher
      loads chosen release    downloads and verifies
                                     |
                                     v
                              Relaunch candidate
                                     |
                              Startup health check
                                /           \
                               v             v
                           Keep it      Restore previous
```

Use object storage, served through a stable HTTPS origin, for released artifacts.
Keep it independent of the Railway application image so releases survive server
replacement. Select the storage provider during the first implementation slice.
Use CI artifacts for build validation; add a durable release publishing step for
player downloads.

### Metadata

Define versioned metadata in a new `internal/release` package and generate it
through `tools/release`.

| Record | Required fields |
| --- | --- |
| Channel pointer | Schema version, channel, increasing promotion generation, target release ID, manifest URL and digest, previous supported release ID, publication time |
| Release manifest | Schema version, release ID, source commit, game version, content revision, minimum launcher version, supported content schema and platform targets |
| Browser artifact | Relative path, SHA-256, decoded byte length and artifact type |
| Desktop target | OS, architecture, archive URL, SHA-256, archive size, expanded size and inventory of installed files |

Record code and content versions separately, while initially distributing them
as one release. Package desktop executables with the assets they were tested
against. Include `game.wasm`, its exact Go `wasm_exec.js`, the filesystem bridge,
and both raylib and Jolt modules in the browser release. Never independently
select code, physics libraries and gameplay assets from different releases.

Generate hashes after packaging. Browser hashes describe decoded response bytes,
not HTTP gzip sidecars; desktop hashes describe archive bytes. Validate metadata
schema, platform, paths, sizes and allowed download origins before installation.
Reserve protocol and save-schema compatibility checks for when those systems
exist; require them before rolling out updates that affect multiplayer or saves.

### Signed desktop updates

HTTPS and checksums alone do not establish that a downloaded native executable
came from an authorized publisher. Before desktop automatic updates ship, choose
a maintained signed-update metadata implementation with pinned root trust,
expiration, key rotation and replay protection. Evaluate The Update Framework
for discovery and download verification. Installation and process recovery still
belong to our launcher. [TUF overview](https://theupdateframework.io/docs/overview/).

Keep signing credentials out of artifacts and limit release signing and stable
promotion to the release workflow. A rollback publishes fresh, increasing channel
metadata that authorizes an older release; accepting arbitrary stale metadata
must not be the rollback mechanism. Platform application signing and update
metadata signing serve separate purposes and both belong in desktop packaging.

## Desktop update flow

For direct downloads, introduce a small `cmd/launcher`. It starts the game from
a complete version directory and manages staged updates. If desktop distribution
uses a game store, use that store's delivery mechanism for those installations;
confirm the distribution choice before building the installer integration.

Proposed layout in a writable per-user installation area:

```text
launcher
state.json
versions/<release-id>/earth-two[.exe]
versions/<release-id>/assets/
staging/<release-id>/
user-data/                              independent of installed versions
```

1. Start the currently selected healthy game without waiting on the update
   service. Check for a compatible release in the background and show its
   version, download size and release notes.
2. Download to staging with timeouts, bounded retries and cancellation. Keep the
   running version untouched. A full verified retry is sufficient initially;
   add byte-range resume after confirming server support.
3. Verify signed metadata, archive size and digest. Extract with traversal and
   unsafe-link protection, enforce expanded-size limits, and validate the
   installed file inventory. Reject wrong-platform packages before execution.
4. Finalize a complete candidate directory on the same filesystem. Journal the
   pending activation and preserve the previous healthy version. Use a tested,
   platform-appropriate atomic state replacement so process or machine failure
   cannot lose the current release selection.
5. Apply the update when the player exits and relaunches, or through an explicit
   restart action. Start the candidate as a child process and require a readiness
   handshake after world loading and the first title frame. Successful download
   or process creation alone does not count as a healthy launch.
6. Mark the candidate healthy after readiness. On an actual startup error or a
   bounded number of failed starts, restore the previous supported version.
   Distinguish slow startup from failure and avoid repeated restart loops.
7. Clean old versions only when they are unused. Retain at least the previous
   healthy release and preserve user data. Treat launcher upgrades as separate
   staged operations handled after the launcher exits; provide manual repair.

Add an explicit asset root to `game.Run` or its configuration. The launcher
passes the selected version's asset directory so an unrelated working-directory
`assets` folder cannot override the release. Preserve current developer-run
fallback behavior. On Windows, install into a new version directory instead of
replacing a running executable; validate activation recovery on every supported
OS and architecture.

## Browser update flow

Keep refresh as the update mechanism. A background check and in-game update
banner are optional; neither a service worker nor background installation is
required for the initial integration.

The useful change is ensuring that one page launch always loads one release.
The current server revalidates files, but that does not make a group of requests
atomic during deployment. Serve builds under immutable release paths:

```text
/                                   resolves stable on a fresh launch
/channels/stable.json
/releases/<release-id>/manifest.json
/releases/<release-id>/index.html
/releases/<release-id>/game.wasm
/releases/<release-id>/raylib.data
/releases/<release-id>/...           remaining generated files
```

The root route resolves the channel and uses a temporary redirect to the selected
release page. All generated relative resource requests remain in that directory.
Preserve query parameters such as `benchmark=1`. Never overwrite a release or
redirect one of its missing resources to the latest build.

Use revalidation for the root redirect and shared channel metadata, and
`public, max-age=31536000, immutable` for released files. Preserve Wasm MIME
handling, gzip sidecars and `Vary: Accept-Encoding`. These cache directives let
immutable files be reused while mutable entry points are checked for changes.
[HTTP cache directives](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control).

Retain old release paths across server deployments for the supported session and
rollback window. A browser tab already running an old release keeps that release;
refreshing the root URL selects the currently promoted release. Because a tab
at a versioned URL remains pinned on refresh, include an “Open latest version”
link or a launch-time channel check on the release page. Perform any automatic
navigation only before the game starts, preserve an explicit recovery pin, and
avoid redirect loops.

Smoke-test the selected release through the serving origin before promotion.
If a bad release reaches stable, promote the previous release and show a startup
error page with a link back to the root. Automatic client crash recovery and
verified offline caches can be added later if needed.

## Progress and compatibility

Contract progress currently lasts only for the running session. An update
restart or browser refresh loses that progress. Returning to the title screen
leaves the session in memory, so it is not a safe automatic restart checkpoint.

Default desktop activation to the next launch. An explicit “Restart to update”
action must explain that current session progress will be lost. Never force a
restart during play for an optional release. If checks run during play, use a
bounded interval with jitter and backoff; update-service errors must not block it.

When persistence is added, confirm saves have flushed before restarting. Keep
saves outside release directories, version their schema and preserve a backup
before migration. A binary rollback cannot safely read a newer save without
explicit compatibility or restore behavior.

World layouts, wardrobe, colliders and gameplay logic activate in a new game
instance. Do not hot-swap them during play. When multiplayer is implemented, the
authoritative server must check supported protocol and gameplay-content versions
at join. Roll out compatible server changes first, clients second, and removal of
old support last. Require incompatible updates before joining and drain old
sessions at a defined boundary.

## Implementation sequence

| Phase | Concrete changes | Acceptance criteria |
| --- | --- | --- |
| 1 Shared packaging and publishing | Add `internal/release`, `tools/release`, clean release staging in the Makefile, per-platform packages and immutable storage; extend CI with preview publishing and a promotion/rollback command | Every artifact is inventoried and hashed; incomplete uploads cannot be promoted; rollback selects existing files |
| 2 Desktop launcher and browser release routing | Add `cmd/launcher`, signature verification, staging and activation journal; add explicit game asset-root configuration and readiness handshake; extend `cmd/web` with release routes and cache rules; adapt `railpack.json` for durable artifact serving | Desktop downloads and relaunches safely; browser refresh selects the promoted complete release; a broken release can be reverted |
| 3 Player controls and release operations | Add desktop release notes, progress, retry/cancel and deferred restart UI in `game/menu.go` and `game/screens.go`; add startup telemetry and explicit beta selection | Optional updates never interrupt play; failures leave the healthy version runnable; beta-to-stable promotion uses the same artifacts |
| 4 Download optimization if needed | Separate content packs from runtime builds and reuse unchanged verified content across releases | A code-only update reuses content; a content-only change downloads only the affected compatible pack |

Phase 2 covers both platforms in the same integration milestone. Keep browser
routing small enough to ship alongside the desktop launcher; do not make browser
caching a prerequisite. Start with manually downloadable desktop packages from
Phase 1 while completing the updater.

For Phase 4, measure compressed transfer size and startup memory first. The
checked-in `assets/` directory occupies approximately 207 MiB on disk, which is
not its network transfer size. Start with world, character and sound packs only
if these measurements justify it. Native installs can reuse verified unchanged
files; browser pack separation needs an illusion build hook and must populate
raylib's shared filesystem before Go starts. Emscripten supports separate preload
packages. [Emscripten packaging documentation](https://emscripten.org/docs/porting/files/packaging_files.html).

True world streaming and binary delta patches are separate later projects.
Existing startup loaders eagerly read world and character content; splitting
packs alone does not make those loaders stream regions during play.

## Validation and rollout

Continue existing vet, test and platform-build checks. Add focused tests for
metadata validation, server routing and cache behavior, staging and activation
recovery. Exercise the browser release paths in Chrome, Firefox and Safari and
test native activation on Linux, macOS and Windows with the architectures we ship.

| Scenario | Required result |
| --- | --- |
| Browser deployment during a slow launch | All requests stay on the selected release |
| Browser refresh after promotion or rollback | Entry route selects the target; pinned release can return to latest |
| Missing artifact, altered bytes or invalid metadata | Candidate rejected; healthy desktop version preserved |
| Interrupted download, cancellation or insufficient disk space | Installation remains recoverable; partial version never activated |
| Candidate crashes before readiness | Previous supported version restored without a restart loop |
| Windows locked executable or power loss during activation | Running version untouched; activation journal recovers |
| Archive traversal, unsafe link or wrong platform | Reject before unsafe extraction or execution |
| Working directory contains unrelated assets | Launcher-selected assets still used |
| Expired, replayed metadata or rotated signing key | Trust policy enforced; fresh authorized rollback works |
| Active contract progress or future incompatible save | No automatic restart; recoverable player data preserved |
| Future incompatible multiplayer release | Unsupported client cannot join the shared world |

Build once, publish to preview, smoke-test, then promote to beta and stable.
Start with explicit beta opt-in; add percentage rollouts only when installation
cohorts and sufficient traffic exist. Track release ID, channel, download duration,
verification failures, startup success, crashes and rollback. Deliver telemetry
to a minimal endpoint or existing service without blocking play. Establish beta
baselines before choosing numerical rollout halt thresholds.

The first implementation slice is the shared manifest, desktop release packages,
immutable browser paths and a working promotion/rollback command. Resolve direct
download versus store distribution and the artifact-storage provider during that
slice, then build desktop staging and recovery against the resulting contract.
