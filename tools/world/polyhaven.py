"""Pieces built on Poly Haven's models (and, for vehicles, Sketchfab's:
see sketchfab.py): real objects, retrofitted, as
docs/look-and-feel.md has it. Poly Haven's models are CC0 (no credit
needed, though tools/world/CREDITS.md keeps a list): photoscanned and
hand-made, with real wear.

A Sourced piece names a model and how to fit it (its height, length or
width in metres, a quarter turn to face -Y), and can add parts of its own
with kit's helpers (stencils, brackets) and colliders as any piece does. The
model is fetched once into build/polyhaven/ and imported; finish.py then
decimates it to its budget and bakes it, crevices and all, onto one texture
like every other piece, so sourced and made pieces look of a piece.

    p = Sourced("drum", "prop", "200 litre drum", asset="barrel_03", height=0.9)
    p.collider((0.6, 0.6, 0.9), (0, 0, 0.45))
"""
import json
import math
import urllib.request
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

import sketchfab
from kit import BUDGET, Piece

ROOT = Path(__file__).resolve().parent.parent.parent
CACHE = ROOT / "build" / "polyhaven"
API = "https://api.polyhaven.com"
AGENT = {"User-Agent": "earth-two world build"}


def _get(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers=AGENT), timeout=120) as r:
        return r.read()


def fetch(asset: str, res: str = "2k") -> Path:
    """The model's glTF, with its textures beside it, downloaded once. A
    "sketchfab:<uid>" asset comes from Sketchfab instead (sketchfab.py)."""
    if sketchfab.ours(asset):
        return sketchfab.fetch(asset)
    folder = CACHE / asset / res
    done = folder / ".done"
    if done.exists():
        return folder / done.read_text().strip()
    files = json.loads(_get(f"{API}/files/{asset}"))
    entry = files["gltf"][res]["gltf"]
    folder.mkdir(parents=True, exist_ok=True)
    name = Path(entry["url"]).name
    (folder / name).write_bytes(_get(entry["url"]))
    for rel, f in entry.get("include", {}).items():
        p = folder / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(_get(f["url"]))
    done.write_text(name)
    return folder / name


def info(asset: str) -> dict:
    """The model's name and authors, from Poly Haven, cached."""
    path = CACHE / asset / "info.json"
    if not path.exists():
        d = json.loads(_get(f"{API}/info/{asset}"))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"name": d["name"], "authors": list(d.get("authors", {}))}))
    return json.loads(path.read_text())


def credits(assets: set[str]) -> str:
    """CREDITS.txt for the models the pieces are built on: Sketchfab's,
    whose licences need the credit, then Poly Haven's."""
    lines = sketchfab.credits({a for a in assets if sketchfab.ours(a)})
    lines += ["World pieces built on Poly Haven models (https://polyhaven.com),",
              "CC0 1.0: public domain, no credit needed, but here it is.", ""]
    for a in sorted(a for a in assets if not sketchfab.ours(a)):
        i = info(a)
        lines.append(f"  {i['name']} by {', '.join(i['authors'])}: https://polyhaven.com/a/{a}")
    lines += ["", "Everything else in assets/world is made by tools/world.", ""]
    return "\n".join(lines)


def load(asset: str, res: str = "2k", drop=()) -> bpy.types.Object:
    """The model imported as one object, its transforms applied. drop
    leaves out its parts whose object or material names have any of those
    words in them (a gun, a cockpit's interior, a display stand)."""
    path = fetch(asset, res)
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(path))
    new = [o for o in bpy.data.objects if o not in before]
    words = [w.lower() for w in drop]

    def dropped(o) -> bool:
        names = [o.name] + [m.name for m in o.data.materials if m]
        return any(w in n.lower() for w in words for n in names)
    for o in [o for o in new if o.type == "MESH" and dropped(o)]:
        new.remove(o)
        bpy.data.objects.remove(o)
    bpy.context.view_layer.update()  # or it still lists what's removed
    meshes = [o for o in new if o.type == "MESH"]
    if sketchfab.ours(asset):
        for o in meshes:
            for m in o.data.materials:
                if m:
                    m["sketchfab"] = True  # finish.py's _glows reads its emissive maps more carefully
    if not meshes:
        raise ValueError(f"{asset} has no meshes")
    for o in bpy.context.view_layer.objects:
        o.select_set(o in meshes)
    bpy.context.view_layer.objects.active = meshes[0]
    # Parents (the glTF's empties) baked into the meshes, then the meshes joined.
    # Shape keys baked to the shape the model's made in: a decimation reads
    # the basis, so a model posed by its keys would move.
    for o in meshes:
        if o.data.shape_keys:
            o.shape_key_clear()
    empties = [o.name for o in new if o.type != "MESH"]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(meshes) > 1:
        bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    # What's left of the import besides it: the glTF's empties.
    for name in empties:
        if name in bpy.data.objects:
            bpy.data.objects.remove(bpy.data.objects[name])
    bpy.context.view_layer.update()  # or it still lists what's removed
    return obj


def fit(mesh: bpy.types.Mesh, fit: dict, rot=(0, 0, 0), turns: int = 0, stretch=(1, 1, 1), at=(0, 0, 0)) -> None:
    """mesh turned (rot, Euler degrees, then turns quarter turns about Z),
    scaled to one size in fit ({"x"|"y"|"z": metres}, the first given; as it
    is if none), then by stretch, with the middle of its foot at at."""
    from mathutils import Euler
    mesh.transform(Matrix.Rotation(turns * math.pi / 2, 4, "Z") @ Euler([math.radians(a) for a in rot]).to_matrix().to_4x4())
    lo = Vector([min(v.co[i] for v in mesh.vertices) for i in range(3)])
    hi = Vector([max(v.co[i] for v in mesh.vertices) for i in range(3)])
    size = hi - lo
    axis, want = next(((a, w) for a, w in fit.items() if w), ("z", size.z))
    k = want / size["xyz".index(axis)]
    centre = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
    mesh.transform(Matrix.Translation(Vector(at)) @ Matrix.Diagonal((*(k * s for s in stretch), 1)) @ Matrix.Translation(-centre))
    mesh.update()
    for p in mesh.polygons:
        p.use_smooth = True


class Sourced(Piece):
    """A piece built on a Poly Haven model. Fit it by one of height, length
    (along X) or width (along Y), in metres, after turning it by turns
    quarter turns (so its front faces -Y), and first by rot (Euler degrees,
    say to lay a rifle on its side); stretch is a further (x, y, z)
    scale, for the odd model a little the wrong shape. budget is the most
    triangles it keeps (finish.py decimates down to it), and recolour paints
    it a palette colour while keeping the texture's wear; paint does that
    for some of its materials ({word in the material's name: colour}), and
    drop leaves parts out (see load)."""

    def __init__(self, name: str, kind: str, note: str = "", *, asset: str, res: str = "2k",
                 height: float | None = None, length: float | None = None, width: float | None = None,
                 turns: int = 0, rot=(0, 0, 0), stretch=(1, 1, 1), at=(0, 0, 0), budget: int = 0,
                 recolour: str | None = None, drop=(), paint: dict[str, str] | None = None):
        super().__init__(name, kind, note)
        self.asset, self.res = asset, res
        self.drop, self.paint = drop, paint or {}
        self.fit = {"z": height, "x": length, "y": width}
        self.turns, self.rot, self.stretch, self.at = turns, rot, stretch, Vector(at)
        self.budget = budget
        self.recolour = recolour

    def build(self, collection: bpy.types.Collection | None = None) -> bpy.types.Object:
        src = load(self.asset, self.res, self.drop)
        mesh = src.data
        # Its own materials repainted one by one: paint maps a word in a
        # material's name to the palette colour it's painted (wear kept).
        for m in mesh.materials:
            for word, colour in self.paint.items():
                if m and word.lower() in m.name.lower():
                    m["recolour"] = colour
        fit(mesh, self.fit, rot=self.rot, turns=self.turns, stretch=self.stretch, at=self.at)
        src.name = self.name
        mesh.name = self.name
        # Its own parts, if it has any, joined on (by Blender's join, which
        # reconciles the two meshes' layers: the model's UVs and the parts'
        # hardened normals).
        if self.bm.faces or self.decals.faces:
            mine = super().build(collection)
            dg = bpy.context.evaluated_depsgraph_get()
            part = bpy.data.meshes.new_from_object(mine.evaluated_get(dg))
            if mine.get("decals"):
                src["decals"] = mine["decals"]
            bpy.data.objects.remove(mine)
            bpy.context.view_layer.update()  # or it still lists what's removed
            parts = bpy.data.objects.new(self.name + " parts", part)
            bpy.context.scene.collection.objects.link(parts)
            if src.name not in bpy.context.scene.collection.objects:
                bpy.context.scene.collection.objects.link(src)
            bpy.context.view_layer.update()  # or it doesn't list the parts yet, and they're left out
            for o in bpy.context.view_layer.objects:
                o.select_set(o in (src, parts))
            bpy.context.view_layer.objects.active = src
            bpy.ops.object.join()
        else:
            self.bm.free()
            self.decals.free()
        for c in list(src.users_collection):
            c.objects.unlink(src)
        (collection or bpy.context.scene.collection).objects.link(src)
        src["kind"] = self.kind
        src["category"] = self.category
        src["source"] = self.asset
        src["budget"] = self.budget or BUDGET[self.kind]
        if self.recolour:
            src["recolour"] = self.recolour
        if self.note:
            src["note"] = self.note
        return src
