# character

The movement half of the Go `character` package on Bevy 0.20 and Avian 3D.

| Rust | Go |
| --- | --- |
| `mod.rs` | `character.go`: `Anim`, `Character`, `Intent`, `Body`, `State`, `Player`; the plugin from `plugin.go` |
| `controller.rs` | illusion `physics.CharacterController` and its Jolt `CharacterVirtual`: a kinematic capsule on Avian shape casts |
| `locomotion.rs` | `locomotion.go` |
| `traversal.rs` | `traversal.go` (ladders are `world::Ladder`, plus `RungSpacing`) |
| `contacts.rs` | `contacts.go`: the pose-fitting geometry; the animation module hands it the sampled pose |
| `health.rs` | `health.go`: the physics of going down and reviving; the ragdoll module adds its rig |

Schedules: `FixedUpdate` runs `prepare_characters → traverse → locomote →
step_characters` in `CharacterSystems::Move`, as illusion ran
`traverse → locomote` and then the physics step. `Update` runs
`player_input` (`CharacterSystems::Input`), `stop_downed`, and `face`
(`CharacterSystems::Act`).

Tests: `apps/client/tests/character.rs` (locomotion, traversal, contacts)
and `apps/client/tests/character_block.rs` (`world/block_test.go` on the
real Hull block).
