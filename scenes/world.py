"""The world's pieces in the 3D pane: the Hull test block as the game lays
it out, or any one part of the world set out piece by piece. The pieces are
built by tools/world; this lays them out and exports them for the pane,
unfinished (make world bakes the finished ones the game gets), and on a
normal build saves the Shape Lab's choices to tools/world/style.json for
make world to build with."""
import math
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools" / "world"))

import bpy  # noqa: E402
from harness_blender import export_glb, frame_all, fresh, parameters, render, report, turntable  # noqa: E402

import build  # noqa: E402
import hull_block  # noqa: E402
from kit import FABRICS, Style  # noqa: E402

BLOCK = "Test block"
defaults = Style.load(build.STYLE)
categories = list(dict.fromkeys(m.CATEGORY for m in build.modules()))
# The Shape Lab lists at most twelve choices: past eleven parts of the
# world, the things carried are shown together.
GROUPS = {c: [c] for c in categories}
if len(categories) > 11:
    for c in ("Items", "Weapons"):
        GROUPS.pop(c, None)
    GROUPS["Items and weapons"] = ["Items", "Weapons"]
p = parameters({
    "show": {"type": "choice", "label": "Show", "default": BLOCK, "options": [BLOCK] + list(GROUPS)[:11],
             "description": "The Hull test block, or one part of the world piece by piece"},
    "repaint": {"label": "Repainted plates", "default": round(defaults.repaint * 100), "min": 0, "max": 100, "step": 5, "unit": "%",
                "description": "How much of the plating, crates and panels have been painted over in another colour"},
    "fabric": {"type": "choice", "label": "Fabric", "default": defaults.fabric, "options": list(FABRICS),
               "description": "The awnings', tarps' and stalls' colours"},
    "seed": {"type": "integer", "label": "Variation", "default": defaults.seed, "min": 1, "max": 999,
             "description": "Which plates get repainted, which colours the crates, containers and goods come in"},
}, title="Earth Two, piece by piece", sources=["scenes", "tools/world"], output="out/model.glb")
style = Style(repaint=p["repaint"] / 100, fabric=p["fabric"], seed=p["seed"])
preview = os.environ.get("HARNESS_DESIGN_PREVIEW") == "1"
show = p["show"] if p["show"] in [BLOCK] + list(GROUPS) else BLOCK
shown = GROUPS.get(show, [])

fresh(scale=1.0)
bpy.context.scene.unit_settings.length_unit = "METERS"


def collection(name: str, parent=None) -> bpy.types.Collection:
    c = bpy.data.collections.new(name)
    (parent or bpy.context.scene.collection).children.link(c)
    return c


pieces = collection("Pieces")
groups = {c: collection(c, pieces) for c in categories}
if preview:
    # A Shape Lab preview builds only what it shows.
    in_block = {name for name, *_ in hull_block.layout()}
    want = (lambda c, n: n in in_block) if show == BLOCK else (lambda c, n: c in shown)
else:
    want = None
built = build.build_pieces(style, groups, want)

# The choices, for make world to build the game's assets with: the pane
# shows the pieces unfinished, which is seconds, where the finish (baking
# every piece) is make world's.
if not preview:
    style.save(build.STYLE)

objs = {name: obj for name, obj, _ in built}
if show == BLOCK:
    # The pieces stay as the originals, out of the export; the block is
    # copies of them sharing their meshes, as the game shares its models.
    block = collection(BLOCK)
    parts: dict[str, bpy.types.Collection] = {}
    for name, x, y, z, turns in hull_block.layout():
        c = objs[name]["category"]
        if c not in parts:
            parts[c] = collection(c, block)
        o = objs[name].copy()
        parts[c].objects.link(o)
        build.place(o, x, y, z, turns)
    hidden = list(objs.values())
else:
    # One part of the world in rows, a metre or so between pieces.
    mine = [o for o in objs.values() if o["category"] in shown]
    hidden = [o for o in objs.values() if o["category"] not in shown]
    bpy.context.view_layer.update()
    width = math.sqrt(sum((o.dimensions.x + 1) * (o.dimensions.y + 1) for o in mine)) * 1.6
    x = y = depth = 0.0
    for o in mine:
        gap = max(0.3, 0.3 * max(o.dimensions.x, o.dimensions.y))
        if x > 0 and x + o.dimensions.x > width:
            x, y, depth = 0.0, y - depth - gap, 0.0
        lo = [min(c[i] for c in o.bound_box) for i in range(2)]
        build.place(o, x - lo[0], y - lo[1] - o.dimensions.y, 0, 0)
        x += o.dimensions.x + gap
        depth = max(depth, o.dimensions.y)
for o in hidden:
    o.hide_render = True
    o.hide_set(True)

frame_all(azimuth=-60, elevation=30)
export_glb("out/model.glb")
render("out/preview.png")
try:
    turntable("out/turntable.mp4", seconds=6)
except Exception as e:  # ffmpeg missing or broken: the model and the still are what matter
    print(f"world: no turntable ({e})")
report()
