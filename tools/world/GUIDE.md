# Making world pieces

How the pieces in `tools/world/` are made, and the bar each has to clear.
Read `docs/look-and-feel.md` and the doc for the piece's part of the world
first: Earth Two is a frontier town built from the parts of a spaceship,
patched, repainted and lived in, with realistic people in it. The pieces sit
beside MakeHuman characters of 40,000-odd triangles with painted textures,
so they have to hold up next to them.

## The bar

A piece is good when it reads as a real, used object at arm's length:

- **A true silhouette.** Real proportions, and no bare boxes standing in for
  things. A crate has a lid, corner castings, ribs and handles. A pipe has
  flanges and brackets. A bench has slats with gaps between them.
- **Secondary forms.** Panel lines (insets 1–2 cm deep), lips and rims,
  hinges, handles, bolts and rivets (cylinders, 8–12 sides), vents and
  louvres, welded patches, cables and hoses that sag (`cable`), and cloth
  that drapes (`cloth`).
- **Wear in the right places.** The finish bakes ambient occlusion, worn
  paint on the edges, grime in the crevices and red dust from the ground
  up. That only works if the shape has edges and crevices to catch it.
- **Real objects from real models.** Anything that exists on Earth (drums,
  rocks, tools, furniture, food, lamps) starts from a Poly Haven model:
  `Sourced` for a whole piece, `p.source(...)` for a part of one. Most
  vehicles and ships start from CC-BY Sketchfab models
  (`asset="sketchfab:<uid>"`, see `sketchfab.py` and `docs/vehicles.md`),
  repainted (`paint`), stripped of what doesn't belong (`drop`) and given
  the story's parts. Other sci-fi and story pieces are modelled.
- **Bodywork is lofted, not cut from plate.** A `prism` is flat on both
  faces and square at its edges, which is right for a bracket and wrong for
  a bonnet. `p.loft` takes cross-sections along X (`kit.section` draws a
  rounded box that can crown, lean in, bulge and keel) and gives a shell
  that tapers and curves. Its `Shell` gives you points on the surface
  (`side`, `top`) to lay parts and painted seams (`p.seam`) on. `p.arch`
  makes wheel arches and fenders, and `kit.rounded` fillets a prism's
  outline so it reads as pressed, not sawn. See the trike in `vehicles.py`.

## Conventions

- Metres, Z up, real-world sizes. The origin is the middle of the
  footprint, on the floor.
- The front faces -Y (the game's +Z, the way the characters face).
  Vehicles' noses point -Y. Guns and blades lie on their side, muzzle or tip
  along +X.
- Architecture is on a 2 m grid with a 3.6 m deck height. **Keep tiling
  pieces exactly on the grid.** A wall is 2.000 m wide, not 2.013: anything
  wider overlaps its neighbour in one plane and flickers. Wall-type pieces
  are centred on y = 0. Wall-mounted pieces have their back at y = 0.
- `kind` is `kit` (architecture), `prop`, `item` (carried) or `vehicle`.
- Each piece has a docstring saying what it is in the world. Use British
  spelling.

## Colliders

- Boxes (`collider`, `solid`; a box can be turned with `rot`) go on
  anything a person would bump into that's bigger than about 0.3 m. Keep
  them inside the visual bounds, and match the walkable surface: a
  catwalk's deck is exactly as deep as its collider.
- Items have no colliders.
- Traversal sizes (`character/traversal.go`):
  - vault: 0.35–1.0 m high and no more than 1.05 m deep
  - mantle: up to 1.8 m
  - slide: under a gap more than 0.9 m high
  - wall-kick: walls about 2 m apart
  - ladder: as `hull_kit.ladder`
- Floors (`deck_floor`, `pad_tile`) have no collider of their own: a level
  stands on one ground box. A floor of boxes side by side has seams that
  catch a slide. Upper decks use `deck_slab`, which has one.

## Never stack surfaces

Two faces of different colours closer than about 5 mm flicker in the game
(z-fighting), and at range or in the browser even 1 cm can. So:

- Paint (stencils, hazard stripes, labels, stains, markings) goes in
  `with p.painted():`, 4–10 mm proud of its surface. It's baked into the
  texture, not left as geometry. `stencil` is always paint. Paint more than
  about 1.5 cm off its surface is out of the bake's reach and never shows.
- A part set into another (glass in a frame, a screen in a bezel) is either
  in its own plane or at least 1 cm proud on every face. It never shares a
  face plane with something of another colour.
- Check with `"$BLENDER_PYTHON" tools/world/zfight.py <module>`.

## Foliage and thin things

Leaves, stalks, grass, wire and mesh go in `with p.plain():`. The finish then
leaves them exactly as built: no bevel, no roughening, which would multiply
their triangles many times over. Build them as flat blades (`prism`) and
crossed cards, a few dozen triangles a plant. Never put foliage on a
`lowpoly()` stand-in: baked onto a card without cut-out transparency, plants
come out as dark slabs.

## Budgets

The engine draws each placed piece separately and without instancing, and
Landfall places about 1,900 of them. `kit.BUDGET` sets the most triangles
per kind, after the finish's bevels:

| kind | budget |
| --- | --- |
| `kit` (tiled by the hundred) | 4,000 |
| `item` | 8,000 |
| `prop` | 20,000 |
| `vehicle` | 60,000 |

A set piece sets its own `p.budget`, up to 150,000: the drifter, the wrecked
terraformer, the crane, the gate. Sourced models are decimated down to
their budget, and baked from their full detail. Tiled kit gets its detail
from its texture, not its triangles.

## The finish

`make world` gives every piece the finish (`finish.py`):

1. Organic parts are roughened: rock, soil, cloth and leaves.
2. Hard parts get a rounded three-segment bevel.
3. The piece is unwrapped and baked onto one texture. The bake adds ambient
   occlusion, edge wear, grime, dust (how much depends on the district:
   Charter Row is clean, the Fringe filthy) and its decals.

A piece ends up as one material and one draw call. Sourced models are baked
from their own textures. `recolour` repaints them a palette colour and keeps
their wear.

## Checking a piece

- `"$BLENDER_PYTHON" tools/world/sheet.py <module> out.png [piece ...] --finish`
  is a contact sheet as the game gets the pieces. Leave out `--finish` for
  a quick look at the shapes.
- `"$BLENDER_PYTHON" tools/world/zfight.py <module>`: nothing listed.
- `go test ./world` checks colliders against bounds, kinds, budgets and the
  test block's traversal.
- To see pieces in the game's own renderer: `go run ./tools/shot out.png
  <asset root> <eye x y z> <target x y z> <file.glb@x,z[,y[,turns]]> ...`
