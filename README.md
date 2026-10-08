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

Native Make targets prepare private dependency copies in `build/deps` using
`tools/deps`. These fix stb_vorbis's offset check before pointer arithmetic
and remove illusion's redundant macOS `libc++` link. Warnings remain enabled;
the shared module cache is unchanged. The copies refresh when dependency
sources change. To use them with a direct Go command, run `make deps` first,
then set `GOWORK="$PWD/build/deps/native.work"` for that command. Browser builds
continue to use illusion's upstream build script.

The browser uses lighter rendering budgets in `game/budget_js.go`: a canvas
at CSS pixel resolution (rather than the display's full pixel ratio), no
MSAA, 1024-pixel shadows, a coarser distant terrain mesh, smaller terrain
textures, and fewer detail and scatter cells. Near terrain geometry and
collision resolution keep their original accuracy. The desktop budgets
are in `game/budget_gl.go`.

Desktop keeps HighDPI, MSAA, 4096-pixel shadows with the full filter, full
terrain detail/textures and cloth simulation. Its performance improvements
reuse static distant-terrain heights, index nearby minimap footprints,
reuse contact-probe buffers, and draw only the live particle triangles.
The shader skips transparency-pattern work on opaque surfaces, preserving
the same coverage for glass and hair. These avoid redundant work without
lowering graphics settings.
Desktop has no software FPS cap; VSync paces it to the display instead of
illusion's default 60 FPS target. Physics continues at its fixed 60 Hz.

`EARTH_TWO_TOUR_STATS=1 EARTH_TWO_HOUR=11 go run ./tools/tour` runs the
desktop tour at a fixed time of day and prints mean FPS and mean/p95 frame
time from the last 60 frames at each view. It uses the normal desktop
graphics settings and VSync. CPU-only comparisons can be run with
`go test ./game -run '^$' -bench 'BenchmarkMinimapSelection|BenchmarkTerrainTileMask' -benchmem`.

The web build animates clothing with the skeleton instead of running the
cloth solver, and uses four shadow-map reads per lit pixel rather than
sixteen. Terrain chunks and stars outside the camera view are skipped.
Terrain rebuilds share height and normal samples between triangles to
avoid repeated noise calculations when crossing chunk boundaries.
F7 cycles shadows through Full, Low and Off while playing; F1 shows the
current setting in the test panel. Off skips the shadow pass entirely for
devices where it costs too much.

For frame measurements, open `/?benchmark=1`. It starts play at the Pads
at 11:00 in clear weather, warms up for 60 frames, then reports FPS, mean
and p95 frame time, CPU time and rendering time every five seconds. CPU
and rendering times measure work submitted by the app, including driver
stalls, rather than GPU execution time alone. On headless hosts without a
working display clock, `/?benchmark=1&timers=1` measures uncapped,
timer-driven frame throughput; its FPS is not display-paced browser FPS.
Normal play installs neither the timing systems nor the benchmark overlay.

`make web` also generates `.gz` sidecars for the JavaScript, WebAssembly and
bundled assets. `cmd/web` serves those when the browser accepts gzip, with
the original content types and conditional revalidation on reload. Other
static hosts must enable gzip encoding for these sidecars or compress the
originals themselves; otherwise they serve the larger original files.
The asset pack is still downloaded and loaded in full before play; these
changes reduce rendering work and transfer size, but do not stream assets.

### Deploying the browser build

`railpack.json` builds and serves the browser build with
[Railpack](https://railpack.com) (Railway uses it): it installs emscripten
(the release CI uses), runs `make web`, and starts `cmd/web`, which listens on
the host's `$PORT`. It builds against the illusion in `go.mod`, so push
illusion and update `go.mod` to it before deploying anything that needs a
newer one.

### Publishing update manifests

[`tools/ota`](tools/ota/README.md) packages tested desktop and browser builds,
uploads immutable downloads to S3, and promotes release manifests in the shared
SpacetimeDB database. Fill the ignored `.env` from `.env.example` for local
publishing. GitHub Actions uses separate secrets and variables; publishing is
disabled until `OTA_PUBLISH_ENABLED=true` is configured. The
[setup guide](tools/ota/README.md) covers storage, publisher authorization, CI and
rollback.

## Controls

The game opens on the title screen: Play, Wardrobe, Controls and (on the
desktop) Quit. Up/Down and Enter, or the mouse, pick from the menus, and
Esc goes back. In play:

| Key | Does |
|---|---|
| WASD / arrows | Walk, relative to the camera |
| Mouse | Look around (in the browser, while a button is held); moving, the camera swings round behind you |
| M | Open or close the map (drag to move it, scroll to zoom) |
| J | Open or close the debt and contract journal |
| P | Testing: teleport to the point under the pointer on the full map, or in play to the marked destination (driving, the vehicle comes too) |
| F1 | Testing: show the test panel; its switches (F5 weather, then the others in turn) step a condition through its settings on demand |
| Shift | Run |
| Space | Jump; vault/mantle an obstacle ahead; kick away from a nearby wall in the air |
| Ctrl | Slide while running (at least 3 m/s); Space jumps out |
| C | Hold to crouch, and walk crouched; let go to stand, once there's room |
| R | Roll in the movement direction, or forward when stationary |
| W/S or Up/Down on a ladder | Climb/descend; release to hold; Space jumps off |
| E | Interact; review the first work order at the arrivals terminal, or deliver its filing at the marked Exchange counter; beside a vehicle, get in; in one, get out (once it's slow); at a bench, stool, chair or bunk, sit down (E or move to get up); at a machine, kneel and work on it (move to stop) |
| H | While driving, toggle the vehicle's headlamps |
| F | Punch |
| Q | Pick up |
| T / G | Talk / dance, until pressed again or until you do anything else |
| Esc | Pause: Resume, Wardrobe, Controls, Main menu, Quit |

What E can use is in `game/use.go`: the seats on each kind of bench,
stool, chair and bunk, and the spot in front of each machine (air
scrubbers, fans, generators, pumps, valve stations, junction boxes,
consoles, conduits, turbines), by piece name. The HUD offers the nearest
within reach, after any vehicle. Sitting goes through the vehicles' seat
(`character.Sit`); working at a machine, talking and dancing are held poses
(`Intent.Hold`), played standing still until the character moves.

### First contract

**First filing** is Ada Vellér's sponsored day-labour work order for an
Unlisted arrival. You start owing her 2,000 marks. Follow the initial marker
to the arrivals terminal at the Pads, press E to read the terms, then choose
**Accept contract** (Enter or click). **Leave it for now** or Esc keeps it
available. Acceptance collects a sealed arrival filing and moves the marker
to a public Registrar counter in the Exchange, in Landfall. Press E there
to hand it over: Ada credits 150 marks, leaving 1,850 owed. There is no bond
or time limit, and this work does not grant a licence or Standing.

J opens a personal account book: a ruled table of debts owed, with creditor,
original amount, repaid and due; one ongoing contract; and a table of completed
receipts showing references, payers, debt credits and totals. Receipts appear
newest first (scroll over the completed section to browse).
In play, a compact top-right summary shows balance and total debt; it matches
the time/weather card's size. Press J for the breakdown. Delivery moves the
job into completed history and reduces debt; this first reward does not add
spendable marks. The task HUD clears after delivery.

A manually marked map destination takes priority over the job marker;
clear that mark to show the job again. For a quick playtest, P in play uses
the current marker: P to the terminal, E and accept, P to the counter, E to
deliver. These are the existing testing teleports, not transport rewards.

The record lasts for the running game, including trips to the main menu;
restarting the game or reloading its browser tab resets it. Persistent
shared-world records and the full filed contract system are still to come.
See [Contract types](docs/contracts.md)
and [The Exchange](docs/exchange.md) for the design and the first order's terms.

In play, a minimap in the top left shows the way round you, north up,
and a compass along the top shows the way the camera looks. M opens the
full map of the Fringe, which pauses the game like the menus. Both maps are
drawn from the layout (`game/maps.go`), so they follow it, and both name
every place they can fit: the seats first, then the roads, then the places
in each seat, as you zoom in.

Landfall stands on terrain (`game/terrain.go`): level inside the dome,
along the roads and under the Fringe's farms, salvage fields, camp and wind
farm, and rolling into dunes and ridges elsewhere, up to hills round the edge
of the world. It's one heightfield for the drawn ground and its collider, and
the layout's pieces are stood on it where they're placed.

The menus (`game/menu.go`, drawn by `game/screens.go`) stack, so Esc or
Back returns to whichever screen opened the one in front. The HUD shows
the frame rate in the bottom left, contextual interaction prompts, current
work under the minimap, and a balance/debt card in the top right. Navigation
bindings are listed in Controls. Text is Inter
(`game/fonts`, SIL Open Font License), embedded in the binary and rasterised
in the screen's own pixels
at about each size it's drawn at (rounded up a little, so resizing the
window loads a few sizes, not hundreds), so it stays sharp at any window
size and pixel ratio, Retina included.

## Vehicles

The buggy, bike, trike, rover and both haulers can be driven: walk up to
one's seat and E gets in, and the keys drive it. W and S are throttle and
brake (and reverse, from a stop), A and D steer (less at speed), and Space
is the hand brake. E gets out at the first of the seat's exits there's room
to stand at, once it's going slower than 4 m/s; on its side or roof, holding
R for a second rights it. The camera follows the vehicle instead, further
back for a bigger one, swinging round behind it as it goes. The HUD shows
the speed and the gear.

Vehicles have forward-facing headlamps that automatically light the road
while driving at night, switching off at dawn or when the driver exits.
H toggles the current vehicle's lamps. Lamps explicitly switched on stay
on after exiting until switched off again; a manual off suppresses the
automatic lights for the current drive. The driving HUD shows their state.

They're Jolt's wheeled vehicles (illusion's `physics.Vehicle`, and its
motorcycle controller for the bike): each wheel finds the ground by itself
on a sprung suspension, and an engine drives them through an automatic
gearbox and differentials. (No anti-roll bars: Jolt's, stiff enough to
keep these bodies flat, rock them side to side on the dunes.) The `vehicle`
package spawns them and drives them. A vehicle is a root physics body with
its chassis model as a child, drawn between physics steps from
`physics.Interpolated`, and its wheels as children of that, posed from
`physics.VehicleState`, so they spin, steer and ride their suspension. The
driver sits in it as a `character.Seated` character: no capsule while it
sits (so it can't bump what it's in), posed at the seat, playing
`Driving_Loop`; shut in the rover's or a hauler's cab, it's hidden.
Astride the bike or the trike, that clip is reshaped as it plays
(`character/ride.go`): the body leans over the tank just as far as it must
for the hands to reach the seat's grips, which they take palms down, and the
feet go back on its pegs (`Piece.seat`'s grips and pegs).

Parked, a vehicle is Static, so it costs nothing and needs no ground
colliders under it (they exist only round the player); getting in makes it
Dynamic. Left alone, it holds its brakes and parks itself again once it's
still, or at once if the ground it's on is about to stream away. Its
springs are as stiff as makes its wheels sink to where they're modelled,
so it doesn't move when it wakes.

How a vehicle is built comes from `tools/world/vehicles.py` (`Piece.wheel`,
`seat`, `chassis` and `drive` in `kit.py`), in world.json under "vehicle".
The Sketchfab models' wheels are cut out of them (every part lying wholly
inside a wheel's cylinder) and exported as wheel pieces of their own
(`polyhaven.wheel_of`). How each kind handles (mass, engine, gears,
suspension, steering, grip) is `vehicle.Handlings`, in Go, so tuning needs
no rebuild.

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
collision capsule. Rolls settle over 0.7 seconds, and slides get up by
themselves over 0.8; still holding Shift and a direction, either runs straight
on out of it. Under a low ceiling, the character stays crouched and
can move slowly, walking crouched (Mixamo's Crouched Walking, a little
lower to pass under the ducts, and Crouching Idle still), until there is
room to stand. Holding C crouches anywhere. There is no stamina cost or
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

The existing Roll clip is joined by dedicated clips made in Blender: ladder
climb/hold, crouch and standing recovery are posed by hand, and the rest are
retargeted from Mixamo downloads in `build/mixamo/` (fetched like the other
Mixamo clips): the slide from `Running Slide.fbx`, which gets up by itself
(still sprinting, it runs on out of it), the two wall kicks from
`Wall Run.fbx`, the vault and mantle from `Vault Over Box.fbx`, as is the hop
off the top of a ladder.
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
  in-air cycle loops it (`Clip.Loop`). Falling on past touchdown, from
  higher than it jumped, it swings its legs (the Fall clip, which
  `traversal.py` makes from the jump's touchdown pose) until it lands.

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

Wardrobe on the title screen or the pause menu opens the wardrobe
(`game/wardrobe.go`): the player stops and turns to the camera, which swings round to their front, and its rows change the body
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
The factions' clothes and the masks are MakeHuman community assets (CC-BY,
credited in `assets/characters/CREDITS.txt`): `make wardrobe` first runs
`tools/makehuman/community.py`, which fetches the ones in `cast.py`'s
`COMMUNITY` into `build/makehuman/` and installs them for MPFB. An item can
be repainted in a faction's colour (a third value in its `CATALOGUE` entry),
keeping its texture's folds and wear.

The slots are face, hair, glasses, mask (a rebreather or dust mask, for the
air outside), top, bottom, outfit (a one-piece), coat (worn over either) and
shoes. `cast.py`'s `LOOKS` are the factions' looks (Crew, Charter,
Registrar, Fringer, Corvane): a whole outfit put on at once, from the
wardrobe's Look row in the game, or by code with `Wardrobe.Wear`.

### The look

The look is painterly realistic: realistic people and objects, lit in soft
bands as if painted. `shading/` draws everything with a toon shader and
outlines only bodies and clothes; its settings are the `shading.Plugin`
values in `game/game.go`. Light falls in soft-edged bands with
violet-tinted shaded faces, except on the ground, which is lit smoothly
(`shading.Smooth`). Cast shadows darken towards black; lamps can still
light them. The fill comes from the sky above and the red soil
below. Light zones change it where the docs say the light isn't the open
air's: filtered and warm under the dome, and the sodium lamps' fill inside
the Hull. Colours brighter than white roll off rather than clip, and haze
thickens with distance. What's less than opaque, clear glass and the edges
of hair, is drawn as that share of its pixels in a fine pattern, so the
dome is seen through without sorting what's drawn. To go with it,
`tools/paint` (`make paint`) gives the textures in `assets/characters` a
painted look, in place. It marks what it paints and skips it next time, so
it's safe to run again.

The sun moves on the real clock (`game/daylight.go`), Landfall time being
UTC: it rises about 05:00, crosses the south no higher than 30° at noon,
and sets about 19:00, with a violet dusk and dawn and a dark night under
the stars, lit by the lamps and beacons and the neighbour planets. The sky
is painted again round the sun as it moves, off the main thread. The night
sky (`game/stars.go`) is about 14,000 stars drawn as points, most faint and
a few bright, coloured by how hot they are and thick along the Milky Way,
with the Milky Way itself (dust lanes and a brighter core) and the airglow
painted into the sky; they wheel round the pole through the night, twinkle,
fade low down, and go behind any dust in the air. In the browser only the
brightest 2,500 are drawn. `EARTH_TWO_HOUR` (0 to 24) holds it at that
hour, and the test panel's Time of day switch does too. The clock under the
minimap (`game/clock.go`) shows the time, the part of the day, the weather,
and the day as a strip with now on it.

The weather is dust (`game/weather.go`), by the real clock, so everyone in
the shared world has the same: mostly clear; now and then (about a tenth of
the time) a dusty spell of an hour or two; and once in a while (a storm
every few days, two in a hundred hours) a dust storm of half an hour to an
hour and a half, blowing up and dying down over minutes. A storm draws the
haze in brown to a couple of hundred metres, puts the sky and the sun
behind the dust, dims the sun and browns the fill, and blows grit across
the view; under the dome the glass keeps most of it out.
`EARTH_TWO_STORM=1` (0 to 1) holds a storm that hard, for looking at one:
`EARTH_TWO_STORM=1 go run ./tools/tour`.

The ground itself is built in `game/terrain.go` from one height function:
dunes, ridges, canyons whose steep sides show their rock in beds, sand
banked against the dome's walls, and the berms that sink the Fringers' hold
into a bowl. Close up, the shader adds the sand's ripples and grain.
`game/scatter.go` streams brush and stones round the player wherever they
go, the same things in the same places each time, off the roads and the
seats' ground.

To see a change to the look, `go run ./tools/tour` runs the game with no
menus and saves a frame at each place in `tools/tour/views.json` (the
Pads, the gate, the Hull, the Exchange, Charter Row, the hold, the haven)
to `out/tour/`, so the same shots can be compared before and after.

### Sound and effects

The game plays nothing it's told to: the cue systems (`game/sound.go`,
`cues.go`, `ambience.go`, `drivesound.go`) watch what the game already
keeps, such as a body's animation, a vehicle's revs, and the menus and the
map, and play what's changed.

- **Ambience:** a bed for each light zone, so sound and light change in the
  same place. Wind outside, rising in a storm, the dome's air handlers, the
  Hull's hum. Over them, the machines, the market and the fountain, each
  heard from the nearest one.
- **Feet:** each body's feet are followed on their bones, and a step plays
  as a foot plants: sand, soil, paving, grating (any kit underfoot) or a
  rug.
- **Bodies:** jumps, landings (as hard as the fall), rolls, slides, wall
  kicks, climbing, punches, sitting, a tool knocking at a machine. E rings
  the Exchange's floor bell.
- **Vehicles:** each kind has its own motor (bike, trike, buggy, rover,
  hauler), pitched by its revs and louder with the throttle. Also tyres,
  skids, doors and crashes.
- **Menus:** paperwork. A tick to move, a stamp to choose, a page to open, a
  pencil on the map, a chime arriving where it's marked.

`EARTH_TWO_CUES=1` prints each cue as it plays.

`game/effects.go` is the dust and the sparks: soft discs facing the camera,
drawn as one mesh rebuilt each frame. Dust comes from steps, landings, rolls
and slides on sand and soil, and from wheels on loose ground. Sparks fly
off metal hit hard.

The sounds are CC0, from Kenney's packs and Freesound. `tools/sounds/fetch.py`
fetches them as `tools/sounds/sounds.json` lists them, cuts, levels and
loops them into `assets/sounds/`, and writes `assets/sounds/CREDITS.txt`.
It needs curl and lame (`brew install lame`). Browsers start sound on the
first click or key press.

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

## The world

Everything in the world is built in Blender Python in `tools/world/`, one
module for each part of the world in the docs. `kit.py` holds the palette
and the piece builder; the modules are:

| Module | Pieces |
| --- | --- |
| `hull_kit.py` | The Hull's middle decks: deck floor, an upper-deck slab, hull walls (one with a viewport), a bulkhead door, ribs, catwalk, railing, stairs, ladder, pipes, a low duct |
| `props.py` | The Hull's market: crates, a drum, an Exchange terminal, a market stall, a tarp, a work lamp, a status light, the Exchange's public board |
| `hull_decks.py` | The Crew's air plant (scrubbers, fans, pumps, the repair-run valve station, generators, a console), the Stacks' rooms (bunks, cots, lockers, curtains, a numbered cabin door), warehouses (racking, pallets, a roller door), food and parts stalls |
| `exchange.py` | The Exchange floor: the Registrars' counter and desks, filing cabinets, a row of terminals, queue rails, the floor bell, archive shelves, the seal, a stamp station, an arbitration table |
| `charter_row.py` | Charter Row: white-and-navy facade walls, windows, doors and pilasters (and their second storeys), roofs and parapets, a glasshouse, planters, hedges, a guard booth, iron gates and fences, lamps, an office (desk, chair, safe, bookshelf), a fountain |
| `pads.py` | The Pads: pad tiles, shipping containers (one open to walk into), a gantry crane, fuel tanks, floodlight towers, a loader, a Quiet Book front, a drifter-day trade table, barriers |
| `domes.py` | The dome line: dome walls, corners and frames, the overhead roof frame and its slopes, strut anchors, the South gate, a boom barrier, the checkpoint booth, a scanner arch, an airlock door, a mask station |
| `fringe.py` | The Fringe: rocks, dust drifts, quiver trees, a dead tree, dry brush, a half-buried ship wreck, a wind turbine, a patched greenhouse, crop beds, the wrecked terraformer, scrap, a salvage rig, a caravan cart, tents, a cistern, fences, a filter cache |
| `items.py` | What's carried: marks, scrip and filters; contracts, filings, ledgers, land claims and seals (and a forged one); water, food and Earth coffee; power cells, parts, valves and a Corvane data core; medicine, seeds, electronics, wine; tools |
| `weapons.py` | Earth guns, lasers, a katana, a machete, a knife, a stun baton, armour, ammo, cell packs, a rack |
| `vehicles.py` | Six-wheel haulers (flatbed and tanker), a bike, a trike, a buggy, a crew rover, a car under a tarp, the drifter *Patience* and the Receivership's shuttle |
| `second_light.py` | The grounded colony ship round the Hull: hull skin sections and corners, the bow and the engine-end stern, rib arches over the streets, a maintenance gantry, cut deck edges, vents, and what stands up through the dome's roof as Landfall's landmark: the bridge tower out of the prow, with its beacon mast, and the stacks over the stern |
| `hull_levels.py` | Between the Hull's decks: ceiling undersides, a hatch with a ladder down, a stairwell to the lower decks, a freight lift, a shaft grate |
| `ground.py` | Roads (straight, corner, junction, end), the salvage track, the gate street, Charter paving, lawn, farm furrows and crop rows, the drifter's landing pad |
| `dressing.py` | District, deck and shop signs with real lettering, Crew marking panels, notice boards, rugs, faction banners, awnings and bunting across streets, doorway curtains |

Real-world objects (drums, rocks, tools, furniture, lamps, food) are built
on Poly Haven's CC0 models (`polyhaven.py`), fetched once into
`build/polyhaven/`. Credits are in `assets/world/CREDITS.txt`. Sci-fi and
story pieces are modelled. How to make a piece, and the standard it has to
meet, is in `tools/world/GUIDE.md`.

`make world` builds every piece into `assets/world/`, each module in its own
process. Every piece gets the finish (`finish.py`):

1. Organic parts are roughened and hard edges bevelled.
2. The piece is baked onto one texture: ambient occlusion, worn edges, grime,
   red dust (as much as its district has) and painted decals. Sourced
   models bake from their own textures.
3. A piece tiled by the hundred (a floor, a wall, a fence) is baked onto a
   low-poly stand-in of a few boxes. The deck floor is 12 triangles that
   still show its grating.

That makes a piece one material and one draw call. The build takes a while;
`make world-fast` builds unfinished pieces in seconds, for trying things.

It writes a `.glb` for each piece and `world.json`, which gives each piece's
model, kind (`kit`, `prop`, `item` or `vehicle`), category, triangle
budget, collider boxes and ladders. It also writes the layouts:
`hull_block.json`, the Hull test block where the game starts, and
`landfall.json`. `make world-layouts` rewrites just those. The `world`
package loads `world.json` and places pieces with their colliders and
ladders.

The conventions every piece keeps (see the guide for the rest):

- Pieces are made in metres at real size. Each stands on its origin (the
  middle of its footprint), and its front faces the game's +Z, the way the
  characters face. Vehicles point their noses that way.
- Guns and blades lie on their side with the muzzle or tip along +X.
- The architecture is on a 2 m grid with a 3.6 m deck height, exactly.
- Floors (deck plates, pad tiles) have no collider of their own: a level
  stands on one ground box under them. Upper decks use `deck_slab`.
- Items have no colliders. Anything big enough to walk into has box
  colliders, sized for traversal (0.35–1 m to vault, up to 1.8 m to mantle).
- Nothing is stacked closer than a centimetre: paint is baked in, never
  layered.

Checking pieces:

- `world`'s tests check that every collider is inside its model, the
  kinds, the triangle budgets, and every move in the test block. That last
  check runs a character under physics with no window: it vaults a crate,
  mantles the tall one, slides under the duct, climbs the ladder, walks up
  the stairs and wall-kicks up the corridor.
- To look a module over:

  ```sh
  "$BLENDER_PYTHON" tools/world/sheet.py fringe fringe.png --finish
  "$BLENDER_PYTHON" tools/world/zfight.py fringe
  go run ./tools/shot shot.png assets 0 2 6 0 1 0 world/drum.glb@0,0
  ```

  `sheet.py` renders a contact sheet; `zfight.py` finds surfaces that would
  flicker; `tools/shot` draws pieces in the game's own renderer.

`scenes/world.py` shows the test block, or any one part of the world piece
by piece, in the Blender harness's 3D pane, unfinished. Its Shape Lab can
change how much is repainted, the fabrics and the variation; a normal run
saves those choices to `tools/world/style.json` for `make world`.

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
- `world/` places the world's pieces; `tools/world/` builds them (see above).
- `tools/shot/` draws world pieces in the game's renderer, for looking them over.
- `tools/sounds/` fetches and cuts the game's sounds (see Sound and effects).
- `tools/paint/` gives the characters' textures a painted look.
- `tools/bindpose/` prepares skinned glTF characters for raylib.
- `tools/mixamo/` puts Mixamo animations on our characters (Blender scripts).
- `cmd/desktop/` runs the game in a desktop window.
- `cmd/web/` is the browser side. Built natively (`go run ./cmd/web`) it's
  the HTTP server for `build/web/`; built for the browser (`make web`) it's
  the game, from `game_js.go`.
- `assets/` holds files the game loads by path. Desktop builds read them from
  disk: from `assets` in the working directory, or else from an `assets`
  folder beside the program (CI's downloads come with one). Web builds bundle
  the directory into the page.

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
