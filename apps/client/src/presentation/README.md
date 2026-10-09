# presentation

How the people of Earth Two look and move on screen: the port of the Go
`character` package's presentation half (animate, skin, outfit, footwear,
appearance, roster, family, presentation, distant, cloth) and the illusion
render pieces it leaned on (animation, bones, pose, skinning, cloth).

The movement half is `crate::character`; this module reads its `Character`,
`Intent`, `CharacterController`, `Traversal`, `Body` and `State`, and runs
in its `Act` set after `face`.

| File | Go source | What it holds |
| --- | --- | --- |
| `anim.rs` | `animate.go` (pure parts) | `Clip`, the thresholds, `pick_anim`, stairs, roll retiming, `air_speed` |
| `player.rs` | illusion `render/animation.go` | the clip clock: loop/once, crossfade on wall time, manual time, `AnimationFinished` |
| `animate.rs` | `animate.go` | the state machine, one body a frame |
| `roster.rs` | `roster.go`, `skin.go` | `Model`, `Skin` (clip fallbacks), `Roster::spawn`, `wear` |
| `presentation.rs` | `presentation.go` | `MotionSamples`: interpolation between fixed steps, feet at the capsule's bottom |
| `family.rs` | `family.go` | the per-frame children index |
| `distant.rs` | `distant.go` | `Distant` lockstep |
| `outfit.rs` | `outfit.go` | slots, `Outfit`, looks, `wardrobe.json`, `dress`, `mirror_pose`, `ModelParts` |
| `appearance.rs` | `appearance.go`, `assetref` | stable asset IDs for remote players |
| `footwear.rs` | `footwear.go` | shoe capsules and cuff clearance |
| `cloth.rs` | `cloth.go` | body capsules, cloth pins, the fit, `clothe` |
| `verlet.rs` | illusion `render/cloth.go` | the solver: 1/60 s steps, at most 2 a frame, 6 passes |
| `bones.rs` | illusion `render/bones.go`, `pose.go` | `Skeleton`, keyframed clips, crossfaded sampling, bone attachments, pose override |
| `mesh.rs` | raylib's Mesh | rendering-free vertex data, `ModelStore` |
| `graph.rs` | raylib's `UpdateModelAnimationEx` | the clip clock on Bevy's `AnimationGraph`: two weighted nodes, Bevy's clocks paused |
| `content.rs` | `game/game.go` | the clip table, the people, the starting outfit |
| `viewer.rs` | `plugin.go`, raylib | GLB loading, scenes, skin tone and hidden meshes, cloth writeback (feature `viewer`) |

Schedules, as in Go: `dress`, `clothe`, `animate` in Update (Act);
`remember_motion` in FixedPostUpdate after the physics writeback;
`present_motion` and the clip clocks in PostUpdate before Bevy's animation,
then `lockstep`, `mirror_pose`, `attach_to_bones`, `apply_players`; the cloth
solver after transform propagation, from the joints' transforms.

`cargo run -p earth-two-client --example presentation` captures
`build/rust/presentation.png`.
