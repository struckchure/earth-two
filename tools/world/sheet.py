"""A contact sheet of some pieces, for looking them over: each piece built
and rendered on its own (Workbench, a three-quarter view from the front,
framed to fit, its name and size under it), and the tiles stitched into a
grid. It writes nothing but the picture.

    $BLENDER_PYTHON tools/world/sheet.py <module> [out.png] [piece ...] [--finish]

module is a module of pieces (e.g. items); naming pieces shows only those.
--finish shows them finished as make world builds them (finish.py): slower,
a bake a piece."""
import math
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bpy  # noqa: E402
import numpy as np  # noqa: E402
from mathutils import Vector  # noqa: E402

from kit import Style  # noqa: E402

TILE = (480, 400)


def _scene() -> bpy.types.Scene:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    s = bpy.context.scene
    s.unit_settings.system = "METRIC"
    s.unit_settings.scale_length = 1.0
    s.render.engine = "BLENDER_WORKBENCH"
    sh = s.display.shading
    sh.light = "STUDIO"
    sh.color_type = "MATERIAL"
    sh.show_object_outline = True
    sh.show_cavity = True
    sh.cavity_type = "WORLD"
    s.world = bpy.data.worlds.new("World")
    s.world.color = (0.55, 0.42, 0.30)
    s.render.resolution_x, s.render.resolution_y = TILE
    s.render.film_transparent = False
    return s


def _tile(make, style: Style, path: Path, category: str = "", finished: bool = False) -> None:
    s = _scene()
    p = make(style)
    p.category = category
    o = p.build()
    if finished:
        import finish
        finish.finish(o, category, p.kind)
        s.display.shading.color_type = "TEXTURE"
    bpy.context.view_layer.update()
    lo = Vector((math.inf,) * 3)
    hi = Vector((-math.inf,) * 3)
    for c in o.bound_box:
        lo, hi = Vector(map(min, lo, Vector(c))), Vector(map(max, hi, Vector(c)))
    centre = (lo + hi) / 2
    radius = max((hi - lo).length / 2, 0.05)
    cam = bpy.data.objects.new("Camera", bpy.data.cameras.new("Camera"))
    s.collection.objects.link(cam)
    s.camera = cam
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = radius * 2.25
    az, el = math.radians(-60), math.radians(25)
    cam.location = centre + Vector((math.cos(el) * math.cos(az), math.cos(el) * math.sin(az), math.sin(el))) * (radius * 4 + 1)
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.clip_start = 0.001
    cam.data.clip_end = radius * 10 + 2
    # The name and size along the bottom, in the camera's frame.
    label = bpy.data.objects.new("Label", bpy.data.curves.new("Label", "FONT"))
    d = hi - lo
    label.data.body = f"{p.name}  {d.x:.2f}×{d.y:.2f}×{d.z:.2f} m"
    label.data.size = cam.data.ortho_scale * 0.05
    label.data.align_x = "CENTER"
    label.parent = cam
    label.location = (0, -cam.data.ortho_scale * TILE[1] / TILE[0] / 2 * 0.92, -1)
    mat = bpy.data.materials.new("Label")
    mat.diffuse_color = (0.05, 0.03, 0.02, 1)
    label.data.materials.append(mat)
    s.collection.objects.link(label)
    s.render.filepath = str(path)
    bpy.ops.render.render(write_still=True)


def sheet(builders, out: Path, columns: int = 0, category: str = "", finished: bool = False) -> None:
    import kit
    if finished:
        import finish
        finish.prepare()
    style = Style()
    tmp = Path(tempfile.mkdtemp())
    tiles = []
    for i, make in enumerate(builders):
        path = tmp / f"{i:03d}.png"
        _tile(make, style, path, category, finished)
        img = bpy.data.images.load(str(path))
        px = np.empty(TILE[0] * TILE[1] * 4, dtype=np.float32)
        img.pixels.foreach_get(px)
        tiles.append(px.reshape(TILE[1], TILE[0], 4))
        bpy.data.images.remove(img)
    columns = columns or min(len(tiles), 5)
    rows = math.ceil(len(tiles) / columns)
    grid = np.ones((rows * TILE[1], columns * TILE[0], 4), dtype=np.float32)
    for i, t in enumerate(tiles):
        # Images store rows bottom up: the first tile goes top left.
        r, c = rows - 1 - i // columns, i % columns
        grid[r * TILE[1]:(r + 1) * TILE[1], c * TILE[0]:(c + 1) * TILE[0]] = t
        grid[r * TILE[1]:(r + 1) * TILE[1], c * TILE[0]] = (0.2, 0.15, 0.1, 1)
        grid[r * TILE[1], c * TILE[0]:(c + 1) * TILE[0]] = (0.2, 0.15, 0.1, 1)
    img = bpy.data.images.new("Sheet", columns * TILE[0], rows * TILE[1], alpha=True)
    img.pixels.foreach_set(grid.ravel())
    img.filepath_raw = str(out)
    img.file_format = "PNG"
    img.save()


if __name__ == "__main__":
    import importlib
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    module = importlib.import_module(args[0])
    out = Path(args[1] if len(args) > 1 else f"{args[0]}.png").resolve()
    finished = "--finish" in args
    names = set(a for a in args[2:] if not a.startswith("--"))
    builders = [b for b in module.PIECES if not names or b.__name__ in names]
    sheet(builders, out, category=module.CATEGORY, finished=finished)
    print(f"sheet: {len(builders)} pieces → {out}")
