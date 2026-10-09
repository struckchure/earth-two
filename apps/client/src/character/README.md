# character

The movement half of the Go `character` package on Bevy 0.20 and Avian 3D.

| Rust | Go |
| --- | --- |
| `mod.rs` | `character.go`: `Anim`, `Character`, `Intent`, `Body`, `State`, `Player`; the plugin from `plugin.go` |
| `controller.rs` | illusion `physics.CharacterController` and its Jolt `CharacterVirtual`: a kinematic capsule on Avian shape casts |
| `locomotion.rs` | `locomotion.go` |
| `traversal.rs` | `traversal.go` (ladders are `world::Ladder`, plus `RungSpacing`) |
| `contacts.rs` | `contacts.go`: the pose-fitting geometry; the animation module hands it the sampled pose |
| `health.rs` | `health.go`: the physics of going down and reviving; incapacitated characters lose their controller and seat |
| `ragdoll.rs` | `ragdoll.go`: dynamic torso, constrained head/limb particles, swept contacts, rolling resistance, settling and wake-up |

Schedules: `FixedUpdate` runs `fall_incapacitated → prepare_characters → traverse → locomote →
step_characters` in `CharacterSystems::Move`, as illusion ran
`traverse → locomote` and then the physics step. `Update` runs
`player_input` (`CharacterSystems::Input`), `stop_downed`, and `face`
(`CharacterSystems::Act`). Ground resistance runs after movement; ragdoll
particles step in FixedPostUpdate after Avian writes back torso transforms.

Tests: `apps/client/tests/character.rs` (locomotion, traversal, contacts)
and `apps/client/tests/character_block.rs` (`world/block_test.go` on the
real Hull block), plus `ragdoll/tests.rs` (bone lengths, collision sweeps,
gentle-slope settling, steep/airborne motion, pause and wake-up).
`apps/client/tests/game.rs` checks repeated critical/fatal recovery from
standing and seated states.
