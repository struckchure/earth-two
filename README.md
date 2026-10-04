# Earth Two

A free-roam role game set in a science-fiction world. You take contracts
and jobs, legal and illegal, earn money and rise through the ranks. It
isn't a racing game.

It's built with [illusion](https://github.com/struckchure/illusion), with Jolt
physics, and the same code runs on the desktop and in the browser.

## Requirements

- Go 1.25 or newer.
- For the desktop: a C toolchain, since raylib compiles with cgo (Jolt
  too; its first build takes about half a minute). On macOS, `xcode-select --install`;
  on Debian/Ubuntu, `gcc g++ libgl1-mesa-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev
  libxinerama-dev libwayland-dev libxkbcommon-dev`; on Windows, MinGW-w64 (e.g. from MSYS2).
- For the browser: [emscripten](https://emscripten.org) (`brew install emscripten`).

## Running

```sh
make run      # opens a desktop window
make serve    # builds for the browser and serves it on http://localhost:8080
```

`make serve PORT=3000` picks another port. To serve an existing build without
rebuilding, run `go run ./cmd/web` (flags: `-addr`, `-dir`).

`make build` writes a desktop binary to `build/`, and `make web` writes the
browser build to `build/web/` (static files you can host anywhere). The first
web build compiles raylib and Jolt with emscripten, which takes a minute;
later builds take seconds.

### Deploying the browser build

`railpack.json` builds and serves the browser build with
[Railpack](https://railpack.com) (Railway uses it): it installs emscripten
(the release CI uses), runs `make web`, and starts `cmd/web`, which listens on
the host's `$PORT`. It builds against the illusion in `go.mod`, so push
illusion and update `go.mod` to it before deploying anything that needs a
newer one.

## Controls

The game opens on the title screen: Play, Wardrobe, Controls and (on the
desktop) Quit. Up/Down and Enter, or the mouse, pick from the menus, and
Esc goes back. In play:

| Key | Does |
|---|---|
| WASD / arrows | Walk |
| Shift | Run |
| Space | Jump; vault/mantle an obstacle ahead; kick away from a nearby wall in the air |
| Ctrl | Slide while running (at least 3 m/s); Space jumps out |
| R | Roll in the movement direction, or forward when stationary |
| W/S or Up/Down on a ladder | Climb/descend; release to hold; Space jumps off |
| E | Interact |
| F | Punch |
| Q | Pick up |
| C | Open or close the wardrobe |
| Esc | Pause: Resume, Wardrobe, Controls, Main menu, Quit |

The menus (`game/menu.go`, drawn by `game/screens.go`) stack, so Esc or
Back returns to whichever screen opened the one in front. The HUD is just
the frame rate and the menu keys. Text is Inter (`game/fonts`, SIL Open Font
License), embedded in the binary and rasterised in the screen's own pixels
at about each size it's drawn at (rounded up a little, so resizing the
window loads a few sizes, not hundreds), so it stays sharp at any window
size and pixel ratio, Retina included.

## Traversal

Move into a ladder toward its rungs to attach automatically. Climbing eases
up to 1.1 m/s, alternates supporting and sliding palms along the side rails,
plants feet on the 30 cm rungs, and holds when input is released. At the top,
a 1.65-second step-over ends with a quick stand. Bottom dismounts take 0.35
seconds, and landing blends back to locomotion in 0.12 seconds. Space while
moving toward a barrier vaults low, narrow obstacles or mantles reachable
ledges when the entire capsule path and landing are clear; otherwise it
jumps normally. While airborne off a sprint, Space near a wall kicks upward
and away: the body comes off the wall like a ball, the way it came in
reflected, and can't steer until it lands. The kick is one foot tapping the wall and bouncing
off it, the last step of Mixamo's Wall Run motion capture without the run up
the wall: the foot nearer the wall reaches back to it and pushes off while
the body turns away into the new flight direction (the clip is mirrored for
a wall on the right), and the body then falls in the kick's last pose, swaying a little, facing
the way it bounced. Touching down it sinks into its knees and stands (Mixamo's
Falling To Landing); it can walk off after a 0.15-second beat. Still holding Shift and a direction,
it skips the landing and runs straight on. Separate presses
can chain kicks between walls; the same surface requires separation before
another kick.

Ctrl slides from a run and R rolls about two metres. Both shorten the
collision capsule. Rolls settle over 0.7 seconds; slides finish with a
0.28-second stand-up recovery that permits movement, jumping, and chaining a
roll. Under a low ceiling, the character stays crouched and
can move slowly until there is room to stand. There is no stamina cost or
invulnerability; actions require fresh presses with a short repeat guard.
Pause and wardrobe menus freeze traversal. Falling below the world resets
movement and the standing capsule.

The starting ground contains a traversal course: a low barrier ahead, a
mantle block to its right, a ladder/platform to the left, a low passage
behind the ladder, and two wall-kick walls to the right of spawn.

`character.TraversalConfig` exposes traversal tuning in metres and seconds;
`character.Traversal` keeps fixed-step state. Authored static ladders use
`character.Ladder` with feet positions, facing, rung spacing and clear exit positions. The
authored clips use 30 cm rung spacing with the first rung 30 cm above the
ladder bottom, with side rails 48 cm apart and 37 cm ahead of the climb axis.
Keep the backing wall at least 65 cm ahead of the climb axis, leaving shoe
and knee clearance behind the rungs. Use these dimensions for ladder geometry. Hands alternate
supporting and sliding up the rails, reversing naturally on descent and
holding when input stops; only feet land on individual rungs. At the top the
body climbs on as its hands leave the rails for the landing, then hops up
onto it; the clip expects the landing's edge 65 cm ahead of the climb axis
and the top exit 1 m ahead. The contact pass
leaves a ladder's own rails and rungs alone while climbing and stepping off.
The local illusion engine supplies capsule overlap/sweep queries,
clearance-tested resizing, and controlled movement without gravity. Slides
and rolls reserve clearance for their leading limbs before stopping. A
post-animation contact pass keeps palms, soles, knees, elbows, and leaning
upper bodies clear of nearby static walls while preserving limb lengths.
The corrected pose is shared by the body, clothing, and bone attachments
on desktop and browser; contact offsets fade out when leaving a wall.

The existing Roll clip is joined by dedicated clips made in Blender: slide,
ladder climb/hold, crouch and standing recovery are posed by hand, and the
rest are retargeted from Mixamo downloads in `build/mixamo/` (fetched like
the other Mixamo clips): the two wall kicks from `Wall Run.fbx`, the vault
and mantle from `Vault Over Box.fbx`, as is the hop off the top of a ladder.
The ladder climb's hands and feet are posed to the rails and rungs, and its
hips and spine move as in `Climbing Ladder.fbx` (by `LADDER_HIPS` in
`cast.py`).
Run `make traversal-animations` to regenerate them for both bodies without
rebuilding meshes or wardrobe assets. The target also checks exported palm orientation, opposing thumb/finger contacts around the
rail section, unchanged bone lengths, planted feet on rungs, slide foot placement,
and loop/recovery seams on both rigs. The normal `make people` pipeline also
includes these clips. Vault/mantle contact phases follow the controller's
lift, cross and landing timing. Ladder playback is driven by distance along
the ladder, including descent and holds. Traversal uses a manually sampled
render clock so physics interpolation and pose playback stay in step while
crossfades continue to blend.

## Characters

`character/` is the character system, an illusion plugin. Every character
shares one body, one locomotion system and one animation state machine, and
differs only in what writes its `Intent`. The keyboard writes the player's;
job and contract AI will write everyone else's.

- A character is a root entity with `Character` (speeds), `Intent`, a
  `physics.CharacterController` and a Transform at the capsule's center.
- It has a child `Body` that holds the model, its `render.AnimationPlayer`
  and a `State`. `Roster.Spawn` builds both.
- `Plugin{Models: ...}` loads the models. A model's `Scale` sizes it; 0 fits
  it to about 1.75 m, for models made in other units.
- The state machine picks Idle, Walk, Run, Jump or RunJump from the
  controller, and plays one-shots (Interact, Punch, PickUp) on request,
  standing or on the move; a skin without a clip ignores the request. A
  moving character pulls up for the action (braking twice as quickly)
  and can't move, turn or jump while it plays, and requests for another
  action are dropped, not queued. Only a fall cuts one short. Still holding
  a direction, it moves on at 70% of the clip, skipping the recovery, into a
  walk that builds back up to a run.
- The body turns at a capped speed, easing in and out (`TurnSpeed`,
  `TurnAccel`), slower the faster it's going, and steps along where it
  faces. It slows the more it has to turn: turning about, it all but stops,
  steps round and sets off again.
- Speed eases toward what's wanted, walking to running and back, starting
  and stopping: each frame it closes a share of the gap (`Ease`), so it
  slows its change as it gets close, capped at `Accel` so a start isn't a
  jolt. It brakes twice as quickly as it speeds up.
- In the air, a character keeps to the way it jumped: it can veer 10° at
  most, going or facing, and can't speed up. Letting go keeps it going.
- Walk and run playback speed follows ground speed, so feet don't slide.
- Jump clips start at take-off (`Clip.Start`) and are stretched so
  take-off to touchdown (`Clip.Land`) lasts as long as the physics jump. A
  model without one holds a pose instead (`Clip.Hold`), and one with an
  in-air cycle loops it (`Clip.Loop`).

The people are a MakeHuman man and woman: realistic bodies (about 29k
triangles, with eyes and eyebrows) in underwear, on MakeHuman's game engine
rig, which has full five-fingered hands. They're built to be customised:
skin, body shape (gender, age, weight, muscle, height, proportions and
ancestry), hair and clothes are all MakeHuman assets. Their everyday moves
(idle, walk, run, jumps, punch, pick up) are Mixamo motion capture; the
rest come from Quaternius's Universal Animation Library: talking idle, jog,
sprint, crouch, roll, interact, punches, push, sit, drive, fixing, pistol,
hit and death. Everything is CC0 except the man's boxer shorts (CC-BY,
credited) and the Mixamo clips (Adobe's terms: fine in a game, not as raw
files); see `assets/characters/CREDITS.txt`.

### The wardrobe

C opens the wardrobe (`game/wardrobe.go`), as does Wardrobe on the title
screen and the pause menu: the player stops and turns to the camera, which swings round to their front, and its rows change the body
(man or woman), the skin tone, hair, glasses, top, bottom, outfit (a
one-piece: suits, overalls, dresses) and shoes. Up/Down (W/S) pick a row
and Left/Right (A/D) change it, or click its arrows; changes show at once.

What a character wears is its `character.Outfit`: a body, a tone and an
item per slot (`Outfit.Put` keeps a one-piece and a top or bottom from
being worn together). The `dress` system puts it on: it swaps the body,
retextures the skin with the tone, and adds each item as a `Garment` child
of the body, posed like it by `mirrorPose`. Clothes are layered: each sits
just over the skin and underwear (and a top over any bottom), so nothing
underneath pokes through. Where the body bends, skin could still show
through as clothes move, so an item that covers nearly all of a body
region (head, torso, hips, upper arms, forearms, hands, thighs, calves,
feet) hides that region's skin, bringing back the little it doesn't cover
as skin patches of its own, drawn in the body's tone. Tops and one-piece outfits replace torso underwear, including bra straps
and cups at open necklines. Hip underwear hides only under an item that
covers it. It all runs
on illusion's `render.ModelParts`, which hides or retextures single meshes
of a model for one entity.

Hair and loose clothes move with physics (`character/cloth.go`, on
illusion's `render.Cloth`): they trail and swing as the character moves,
keep their folds rather than crumpling, and at rest hang as they're
modelled. They're pushed out of capsules fitted to the body, a few along
each limb and down the trunk, each sized to its own stretch of it (a single
capsule would be the thigh's width all the way to the knee, and push the
trousers out there).
What touches the skin stays where the animation puts it; fitting measures
distance to body triangles so the middle of a face is held too. The further
along a garment from there, the further a part can stray. Hair, tops and
trousers have separate damping and bending settings. Trouser cuffs also
collide with upper surfaces fitted to the selected shoes, including their
bone weights; switching footwear rebuilds the fit. Only fabric near the
shoes gets extra clearance, so shorts and bare feet retain their usual fit. Tops hold their shoulder
and neckline band to the animated pose so their supports do not float. Glasses and shoes are
rigid.

Cloth renders an interpolated displacement from the current animated pose
between its 60 Hz physics steps, and its shading normals follow the moving
folds while preserving the authored smoothing across texture seams. Character
bodies and the following camera also interpolate the last two controller
positions (`character/presentation.go`); physics keeps its actual position,
and respawns appear immediately. Textures use mipmaps and trilinear filtering
to reduce shimmer, and animation fades use elapsed time independently of
stride playback speed. These renderer changes currently live in the local
`../illusion` checkout selected by `go.work`; the browser builder uses that
same engine checkout for its Go code and C bridges.

`tools/makehuman/wardrobe.py` (`make wardrobe`, after `make people`) builds
it: it fits every item in `cast.py`'s `CATALOGUE` to each body, drapes what's
worn on the trunk (MakeHuman fits clothes to the skin, into the waist and
the small of the back; draped, a shirt hangs from the chest and shoulder
blades straight down to the hips, and bridges the spine and the cleavage),
layers it over what it's worn over (a shirt over a waistband bulges out
smoothly, rather than taking the shape of every belt loop), measures which regions and underwear it covers
(casting rays outward), exports each
item with its skin patches to `assets/characters/<body>/<slot>/`, writes the
skins to `assets/characters/skins/`, and lists it all in
`assets/characters/wardrobe.json`, which the game loads. The items are the
MakeHuman system hair, shoes and suits, and the shirts01, pants01 and
glasses01 packs, all CC0: install those three packs like the others below.

### The look

`shading/` draws everything with a toon shader (light in hard bands, tinted
shadows, a rim of light) and outlines bodies and clothes; its settings are
the `shading.Plugin` values in `game/game.go`. To go with it, `tools/paint`
(`make paint`) gives the textures in `assets/characters` a painted look, in
place. It marks what it paints and skips it next time, so it's safe to run
again.

### The character pipeline

Everything about who the characters are is in one file,
`tools/makehuman/cast.py`, and one command builds `assets/characters` from
it:

```sh
make characters
```

`cast.py` has the people (`PEOPLE`), the face they share (`FACE`), the
faces, clothes and skins the wardrobe offers (`FACES`, `CATALOGUE`,
`SKINS`), the clips they get (`CLIPS`, `MIXAMO_CLIPS`) and how the
hips move on a ladder (`LADDER_HIPS`). `make characters` runs
`make people` (which authors the traversal clips too), then `make
wardrobe`, then `make paint`. After changing only the traversal clips,
`make traversal-animations` redoes just those on the bodies as they are.
Each step runs on its own too, and the first two paint what they build
(`PAINT=0` leaves the textures as MakeHuman's). A step that fails stops the
build.

`FACE` is MakeHuman targets and how much of each. Each of `FACES` is the
targets that differ from it, and `make wardrobe` builds it as a head worn in
place of the body's. They can change the features but not the head's or neck's shape,
so the neck stays closed and the hair still fits.

### Building the people

`tools/makehuman/people.py` builds them in Blender (headless) with
[MPFB](https://extensions.blender.org/add-ons/mpfb/), MakeHuman for Blender:
it makes each body, rigs it, retargets the library's and Mixamo's clips
onto it and exports a `.glb`. The library's skeleton has the same
Unreal-mannequin bone names as the game engine rig, and Mixamo's maps onto
it by `MIXAMO_BONES`, so clips carry over bone by bone, relative to a
reference pose (the MakeHuman rig posed to match the source's T-pose).
Mixamo clips are turned to face the way MakeHuman does and lose their hips'
net travel, so they play in place.
Faces the texture makes see-through, the eyes' corneas, are dropped, since
raylib would draw them invisible but still hide the eyeballs.

The spine, neck and head keep MakeHuman's own rest pose as their reference
(both skeletons stand upright at rest, but each spine sits in its body
differently, so matching them joint for joint leans the body), and so do
the clavicles (the library's T-pose holds its shoulders back, which would
push MakeHuman's chest out); only the limbs are lined up with the library's. MakeHuman's spine is proportioned
differently, so the same spine rotations would lean its torso further; each
frame, the spine is turned so the torso leans as much as the source's.

The library's walking and running clips need more (the game uses Mixamo's
instead, whose motion capture carries over as it is): the library leans a moving body by tilting
the pelvis and bending the spine back against it, which on MakeHuman (whose
pelvis carries the belly and buttocks) arches the lower back. For the clips
in `LEVELLED`, the pelvis, spine, neck and head keep only their movement
around their average, MakeHuman's upright rest, and lean forward together
from the hips by a set amount (none walking, 6° jogging, 12° sprinting).
The library's `Idle_Loop` is a
fighter's ready stance, so the script also makes `Idle_Relaxed`, the game's
idle: that clip's breathing in the spine, neck and head (its average lean
taken out), with the legs, pelvis and shoulders at ease and the arms
hanging.

Setup, once. Everything lives in the git-ignored `build/makehuman/`, and
Blender runs with its own user folder there, so your Blender is untouched:

1. Download [MPFB](https://extensions.blender.org/add-ons/mpfb/) and the
   [MakeHuman system assets](https://static.makehumancommunity.org/assets/assetpacks/makehuman_system_assets.html)
   (CC0) into `build/makehuman/dl/` as `mpfb.zip` and `system_assets.zip`,
   and the [Universal Animation Library](https://quaternius.itch.io/universal-animation-library)
   (Standard) unzipped into `build/makehuman/dl/ual/`, plus the
   [underwear01](https://static.makehumancommunity.org/assets/assetpacks/underwear01.html)
   pack (CC0) as `underwear01.zip`, and for the wardrobe the
   [shirts01](https://static.makehumancommunity.org/assets/assetpacks/shirts01.html),
   [pants01](https://static.makehumancommunity.org/assets/assetpacks/pants01.html) and
   [glasses01](https://static.makehumancommunity.org/assets/assetpacks/glasses01.html)
   packs (CC0) as `shirts01.zip`, `pants01.zip` and `glasses01.zip`.
2. Install MPFB and the assets:

   ```sh
   export BLENDER_USER_RESOURCES=$PWD/build/makehuman/blender
   blender --factory-startup --command extension install-file --repo user_default --enable build/makehuman/dl/mpfb.zip
   for pack in system_assets underwear01 shirts01 pants01 glasses01; do
     blender -b --python-expr "import bpy; bpy.ops.mpfb.load_pack(filepath='$PWD/build/makehuman/dl/$pack.zip')"
   done
   ```

3. Install WojackOWL's [Boxer Shorts](http://www.makehumancommunity.org/clothes/boxer_shorts.html)
   by hand: download its `.mhclo`, `.obj`, `.mhmat`, thumbnail and base
   colour texture into
   `build/makehuman/blender/extensions/.user/user_default/mpfb/data/clothes/wojackowl_boxer_shorts/`,
   rename the `.mhclo` and thumbnail to `wojackowl_boxer_shorts.*`, and
   delete the `.mhmat`'s bump, normal and displacement lines (their
   textures aren't needed).
4. Download the Mixamo clips named in `MIXAMO_CLIPS` from mixamo.com (any
   Mixamo character: their skeletons are the same) as FBX Binary, **Without
   Skin**, 30 fps, into `build/mixamo/`, named after the clip
   (`Walking.fbx`). `build/` is git-ignored: Adobe's terms don't allow
   sharing the raw files.

Then `make people` rebuilds `assets/characters/man.glb` and `woman.glb`
(`BLENDER=` points at another Blender). Who they are (gender, skin), which
clips they get and what they wear is in `cast.py`; their clip
names for the game are `makehuman` in `game/game.go`.

### Animating a character with Mixamo

`tools/mixamo` runs in Blender (headless; the add-on isn't needed):

1. Prepare the mesh for Mixamo's auto-rigger. This writes it alone, T-posed,
   1.75 m tall and facing front:

   ```sh
   blender -b --python tools/mixamo/upload.py -- path/to/model.glb build/mixamo/man-for-mixamo.fbx
   ```

2. On mixamo.com, upload it, place the markers, and download each animation
   as FBX Binary, **Without Skin**, 30 fps, **In Place** where offered, into
   `build/mixamo/` (git-ignored: Adobe's terms don't allow sharing the raw
   files). The file name becomes the clip name.
3. Merge the clips onto the mesh, then fix the skeleton for raylib:

   ```sh
   blender -b --python tools/mixamo/merge.py -- build/mixamo/man-for-mixamo.fbx build/mixamo/mixamo-man.glb build/mixamo/Walking.fbx ...
   go run ./tools/bindpose -o assets/characters build/mixamo/mixamo-man.glb
   ```

`merge.py` welds the flat-shaded mesh so Blender's automatic weights can
bind it to Mixamo's skeleton (faces stay flat), removes each clip's net
horizontal hip travel so everything plays in place, and exports one `.glb`.
Set `Character.WalkSpeed` and `RunSpeed` to the walk and run clips' own
speeds, or the feet slide.

### Preparing models for raylib

raylib reads skinned glTF differently from the spec. It takes a skeleton's
bind pose from its bones' rest transforms, ignoring the inverse bind matrices,
so FBX-exported models whose rest pose differs from their bind pose skin
their arms wrongly. It also bakes the skinned mesh's own transform into the
vertices, so a Blender export (mesh under an armature scaled 0.01) comes out
tiny when unposed. `tools/bindpose` fixes both. To add a model:

```sh
make bindpose SRC="path/to/model.glb"
go run ./tools/bindpose -check assets/characters/model.glb   # every joint should be off by ~0
```

Then add it to `people` with its clip names.

## CI

`.github/workflows/ci.yml` vets, tests and builds the game for Linux, macOS
and Windows, and builds it for the browser, on pushes to `main` and on pull
requests. Each run uploads the builds as artifacts.

## Layout

- `game/` is the game: plugins, a startup system that spawns the scene, and
  update systems. See illusion's README for how systems, queries and
  resources work.
- `character/` is the character system (see above).
- `shading/` is the toon shader and the outlines.
- `tools/paint/` gives the characters' textures a painted look.
- `tools/bindpose/` prepares skinned glTF characters for raylib.
- `tools/mixamo/` puts Mixamo animations on our characters (Blender scripts).
- `cmd/desktop/` runs the game in a desktop window.
- `cmd/web/` is the browser side. Built natively (`go run ./cmd/web`) it's
  the HTTP server for `build/web/`; built for the browser (`make web`) it's
  the game, from `game_js.go`.
- `assets/` holds files the game loads by path. Desktop builds read them from
  disk, relative to the working directory, so run the game from here. Web
  builds bundle the directory into the page.

## Notes for the browser

- The game fills the page and renders at the screen's full resolution
  (illusion's `window.Config` with `Resizable` and `HighDPI`). 2D drawing is
  in device pixels there, so scale sizes by `window.Window.Scale`, as `hud`
  does.
- Everything in `assets/` is downloaded before the game starts, so keep it to
  what the game needs.
- Browsers keep sound off until the player clicks or presses a key.
- Web builds use a browser version of raylib-go that covers the functions
  illusion and its examples use. A raylib function it doesn't have yet fails
  the web build with `undefined: rl.X`.
