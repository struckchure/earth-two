"""Finds surfaces that would flicker: two faces of a piece facing the same
way, closer than a few millimetres apart and overlapping. The depth buffer
can't tell which is in front, so the game shows them fighting, worse at
range (and worse again in the browser).

    $BLENDER_PYTHON tools/world/zfight.py [module ...]

It checks what the game draws: a piece's low-poly stand-in if it has one,
and without its paint (the finish bakes paint in). world's tests don't run Blender, so this is the check to run after
changing a piece."""
import importlib
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bpy  # noqa: E402  (before bmesh, which needs it)
import bmesh  # noqa: E402

import build  # noqa: E402
from kit import Style  # noqa: E402

GAP = 0.004  # metres: closer than this, two faces fight


def fights(obj: bpy.types.Object, gap: float = GAP) -> list[tuple]:
    """Pairs of nearly coplanar, overlapping faces of different colours from
    different parts of obj: (where, how far apart, the area they share, in
    m²)."""
    bm = bmesh.new()
    dg = bpy.context.evaluated_depsgraph_get()
    bm.from_mesh(obj.evaluated_get(dg).to_mesh())
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.faces.ensure_lookup_table()
    # Which connected part each face is in: a box's own faces never fight.
    part = {}
    for f in bm.faces:
        if f in part:
            continue
        stack, n = [f], len(part)
        part[f] = n
        while stack:
            g = stack.pop()
            for e in g.edges:
                for h in e.link_faces:
                    if h not in part:
                        part[h] = n
                        stack.append(h)
    planes = defaultdict(list)
    for f in bm.faces:
        if f.calc_area() < 1e-6:
            continue
        n = f.normal
        key = tuple(round(c, 2) for c in n)
        planes[key].append(f)
    found = []
    for faces in planes.values():
        faces.sort(key=lambda f: f.normal.dot(f.verts[0].co))
        for i, a in enumerate(faces):
            da = a.normal.dot(a.verts[0].co)
            for b in faces[i + 1:]:
                db = b.normal.dot(b.verts[0].co)
                if db - da > gap:
                    break
                # The same colour fighting itself looks the same either way.
                if part[a] == part[b] or a.material_index == b.material_index or a.normal.dot(b.normal) < 0.999:
                    continue
                area = _overlap(a, b)
                if area > 1e-5:
                    found.append((tuple(round(c, 3) for c in a.calc_center_median()), round(db - da, 4), area))
    bm.free()
    return found


def _overlap(a, b) -> float:
    """Roughly the area two nearly coplanar triangles share: b's triangle
    clipped to a's, in a's plane."""
    from mathutils.geometry import intersect_tri_tri_2d  # noqa: F401
    n = a.normal
    u = (a.verts[1].co - a.verts[0].co).normalized()
    v = n.cross(u)
    o = a.verts[0].co
    pa = [((p.co - o).dot(u), (p.co - o).dot(v)) for p in a.verts]
    pb = [((p.co - o).dot(u), (p.co - o).dot(v)) for p in b.verts]
    poly = pb
    for i in range(3):
        (x1, y1), (x2, y2) = pa[i], pa[(i + 1) % 3]
        side = lambda p: (x2 - x1) * (p[1] - y1) - (y2 - y1) * (p[0] - x1)  # noqa: E731
        sign = 1 if side(pa[(i + 2) % 3]) > 0 else -1
        out = []
        for j in range(len(poly)):
            p, q = poly[j], poly[(j + 1) % len(poly)]
            sp, sq = sign * side(p), sign * side(q)
            if sp >= 0:
                out.append(p)
            if (sp >= 0) != (sq >= 0):
                t = sp / (sp - sq)
                out.append((p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])))
        poly = out
        if len(poly) < 3:
            return 0.0
    return abs(sum(poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1]
                   for i in range(len(poly)))) / 2


def main(names: list[str]) -> int:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    # Checked as the finish builds them: a stand-in rather than the model
    # baked onto it, and paint apart (it's baked in): what the game draws.
    import kit
    kit.FINISHING = True
    bad = 0
    for m in build.modules():
        if names and m.__name__ not in names:
            continue
        for make in m.PIECES:
            p = make(Style())
            o = p.build()
            if o.get("source"):
                continue
            found = fights(o)
            if found:
                bad += 1
                area = sum(a for *_, a in found)
                worst = min(g for _, g, _ in found)
                print(f"{p.name}: {len(found)} pairs, {area:.3f} m², closest {worst * 1000:.1f} mm, e.g. at {found[0][0]}")
    print(f"zfight: {bad} pieces with surfaces that fight")
    return bad


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    sys.exit(1 if main(args) else 0)
