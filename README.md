# Earth Two

A free-roam role game set in a low-poly Lagos. You take contracts and
jobs, legal and illegal, earn money and rise through the ranks. It isn't a
racing game: the danfos, okadas and crowds are there to live among.

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

## Controls

| Key | Does |
|---|---|
| WASD / arrows | Walk |
| Shift | Run |
| Space | Jump |
| E | Interact |
| F | Punch |
| Q | Pick up |
| Tab | Switch between the man and the woman |

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
  controller, and plays one-shots (Interact, Punch, PickUp) on request. Moving
  cuts a one-shot short, and a skin without a clip ignores the request.
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
   pack (CC0) as `underwear01.zip`.
2. Install MPFB and the assets:

   ```sh
   export BLENDER_USER_RESOURCES=$PWD/build/makehuman/blender
   blender --factory-startup --command extension install-file --repo user_default --enable build/makehuman/dl/mpfb.zip
   for pack in system_assets underwear01; do
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
clips they get and what they wear is at the top of `people.py`; their clip
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
   blender -b --python tools/mixamo/merge.py -- build/mixamo/man-for-mixamo.fbx build/mixamo/man.glb build/mixamo/Walking.fbx ...
   go run ./tools/bindpose -o assets/characters build/mixamo/man.glb
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
make characters SRC="path/to/model.glb"
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
