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
| `sound.rs`, `sound_output.rs` | `sound.go`: camera listener, loop envelope, original audio decoding and raylib stereo pan law |
| `sound_web.rs` | Browser CPAL stream ownership and user-gesture activation, retaining the same Bevy sinks and decoder |
| `cues.rs` | `cues.go`: foot plants, body transitions, world/UI gains and pitch variation |
| `effects.rs` | `cues.go`/`drivesound.go`: gameplay particle emission and surface/zone rules |
| `drive_sound.rs` | `drivesound.go`: engine/tyre loops, entry/exit, skid and collision cues |
| `ui_sound.rs` | `cues.go`: current title/pause focus, navigation and HUD denial cues |
| `sound_hits.rs` | `sound.go` and illusion `audio.go`: clip variants, finite playback, eight voices per variant and cleanup |
| `ambience.rs` | `ambience.go`: zone beds, storm/driving wind, nearest machines and menu ducking |
| `menu.rs`, `wardrobe.rs`, `wardrobe_render.rs` | `menu.go`, `wardrobe.go`: stacked pages/focus, live outfit rows, front preview and W-1 form |
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

Ambience follows the game camera after its update. It preserves the Go zone
weights, source distances, 0.55 mix gain, 0.4 menu duck and exponential fades.
Eight WAV loops share the original files; OGG is a fallback. The Bevy audio
adapter restarts a decoder at EOF, retaining live panning without buffering a
whole decoded loop. Finished fades despawn their voice; returning to an area
creates one replacement. Missing/invalid audio stays silent. Native cue logs
use `EARTH_TWO_CUES=1`; browser cue logs use `?cues=1`.

Footsteps read the body's own rendered `foot_l`/`foot_r` joints after pose fitting
and transform propagation. Memories reset when the skeleton changes and are
removed when bodies despawn. The Go 3.5 cm plant/6 cm lift thresholds, 0.22 s
minimum gap, gait checks and crouch gain remain unchanged. Body cues observe
animation/grounding transitions. Vehicle loops use each handling family's Go
pitch/idle/throttle values; pause and exit fade them to silence.

One-shots discover bare names before contiguous numbered variants, preferring
OGG then WAV. Expected missing-file probes terminate discovery; Bevy logs these
as missing paths. Each variant permits eight overlaps; when full, slot zero is
restarted as in illusion's `freeVoice`. Finished and unavailable-device voices
release their entities/assets. UI sounds are integrated only for the playable
menu/HUD; map and bell interaction cues await those flows. Wardrobe changes now use
the original cloth cue, with page/back/stamp sounds based on stack depth.
Particle emission now shares those exact foot/body/vehicle triggers. It uses
`sky::Effects` (the existing `effects.go` simulation), with foot dust excluded
inside the dome, landing/roll/slide dust on loose surfaces, fractional wheel
emission and crash rings/sparks. The emission clock follows Go even when menus
pause physics. Particle motion runs after all emissions in PostUpdate, then
the renderer updates its single sorted mesh. Paired listening/visual comparison
and platform acceptance remain open.

The wardrobe is available from title and pause. `menu::Menu` owns the page
stack and each page's focus. The headless input/action systems run before
character input; the rendered buttons write the same actions. Body, tone,
look and all nine slots change the player's actual Outfit. The presentation
systems replace and bind its garments while physics is paused. `face_camera`
runs after character/seat posing; the camera uses Go's fixed front shot in
the wardrobe and continues the underlying title orbit clock. Tests in
`tests/wardrobe.rs` and `tests/game.rs` cover the source rules and full keyboard
flow; `examples/wardrobe_smoke.rs` verifies rendered outfit replacement and
walking afterward. Full title/pause styling and other pages remain pending.
