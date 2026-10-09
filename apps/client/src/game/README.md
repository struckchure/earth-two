# game

The playable application and Go game-package integration on Bevy.

| Rust module | Go source / responsibility |
| --- | --- |
| `mod.rs` | `game.go`, `menu.go`: loading/title/play/pause, session, player and subsystem ordering |
| `camera.rs` | `camera.go`, `drive.go`: orbit, chase and collision arm |
| `seats.rs` | `drive.go`, `character/seat.go`: offers, input, entry, seating and exit |
| `injuries.rs` | `injuries.go`: downed driver release, R recovery and injury text |
| `residents.rs` | `residents.go`: resident data, nearest-character selection, wandering, health persistence and cleanup |
| `crowd.rs` | `testing_npcs.go`: F8/F9 controls, batched local placement, outfit selection and relocation |
| `people.rs` | `cull_people.go`: connects `landfall/cull_people.rs` rules to live character hierarchies |
| `render.rs` | Platform shell, asynchronous loading and temporary menu/HUD/review panels |

Resident updates run before character input; crowd keys run before menu keys.
The test crowd is off by default, as in Go. `--at=crowd` or `/?at=crowd`
enables 25 residents. F8 toggles; F9 edits 0–4199; Enter applies; Esc cancels;
F1 shows/hides this review panel. This increment ports the NPC controls, not
the remaining Go test-panel settings or full UI styling.

Up to 256 resident records are placed each frame. Up to 16 full characters are
created each frame, selecting the nearest 400 within 55 m, retaining up to 20
extra existing actors and releasing beyond 65 m. Distant healthy residents
move as data in eight slices. Critical/dead residents remain still and return
prone with their health and ragdoll. Clears advance a generation so old actors
cannot be mistaken for new records at the same index.

`tests/residents.rs` covers Go lifecycle, population editing, limits and injury
streaming. `tests/game.rs` checks real Landfall walking and menu/input ordering.
`examples/crowd_smoke.rs` exercises the native renderer and saves captures under
ignored `build/migration-baseline/step-4-residents/`.
