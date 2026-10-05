"""Charter Row: the Charter Families' street of old company buildings, kept
clean (docs/settlement.md, docs/factions.md). White walls, company navy,
stone, gardens under glass and private guards: the only straight lines in
town, and it should look it, unlike everywhere else.

The kit is on the Hull's 2 m grid with a 3.6 m storey; walls are 0.3 m
thick (0.4 with their mouldings), exactly 2 m wide and centred on y = 0, their
street face looking along -Y. They're tiled by the dozen, so each has a
low-poly stand-in (Piece.lowpoly) that the game draws, its mouldings, joints
and frames baked onto it. Props stand on their origin, their front along -Y.
Real objects (lamps, chairs, the bookshelf, the gate, a planter, a potted
plant) are Poly Haven's (see polyhaven.py)."""
import math

import bpy

from kit import DECK, GRID, PALETTE, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "Charter Row"

PALETTE.setdefault("Leather green", (0.04, 0.12, 0.06))
PALETTE.setdefault("Ink", (0.01, 0.01, 0.03))
PALETTE.setdefault("Terracotta", (0.45, 0.14, 0.06))
PALETTE.setdefault("Citrus", (0.85, 0.40, 0.02))
# Clipped greenery: leafy surfaces made here (_clipped), not roughened again
# by the finish, which would melt a hedge's square edges.
PALETTE.setdefault("Clipped green", (0.05, 0.17, 0.04))
PALETTE.setdefault("Clipped light", (0.12, 0.27, 0.06))

H = GRID / 2
# Walls side by side overlap their colliders this much (see hull_kit).
SEAM = 0.02
WALL = 0.12  # half the body's thickness

# The bands every Charter wall shares, bottom to top, each its own slab so
# none passes through another: a rusticated navy plinth, a white string
# course, the white body, a navy frieze, a moulded white cornice and a
# stone coping.
PLINTH, STRING, FRIEZE, CORNICE, COPING = 0.55, 0.62, DECK - 0.38, DECK - 0.22, DECK - 0.08
# Their sections across the wall (y, z), symmetric: both faces are finished.
STRING_PROFILE = [(-0.165, 0), (0.165, 0), (0.165, 0.025), (0.155, 0.05), (0.135, 0.07),
                  (-0.135, 0.07), (-0.155, 0.05), (-0.165, 0.025)]
CORNICE_PROFILE = [(-0.135, 0), (0.135, 0), (0.15, 0.02), (0.165, 0.05), (0.18, 0.08), (0.195, 0.1), (0.2, 0.12),
                   (0.2, 0.14), (-0.2, 0.14), (-0.2, 0.12), (-0.195, 0.1), (-0.18, 0.08), (-0.165, 0.05), (-0.15, 0.02)]
CORNICE_LOW = [(-0.135, 0), (0.135, 0), (0.2, 0.11), (0.2, 0.14), (-0.2, 0.14), (-0.2, 0.11)]


def _run(p: Piece, profile, x0, x1, z, mat) -> None:
    """A moulding: profile (y, z) run along X from x0 to x1, at height z."""
    p.prism([(y, z + dz) for y, dz in profile], x1 - x0, ((x0 + x1) / 2, 0, 0), mat, rot=(0, 0, 90))


def _plinth(p: Piece, x0, x1) -> None:
    """The plinth from x0 to x1: a navy core, and rusticated blocks in two
    staggered courses proud of it, a centimetre's joint between them."""
    p.span((x0, -0.14, 0), (x1, 0.14, PLINTH), "Charter navy")
    for course, (za, zb) in enumerate(((0, 0.27), (0.28, PLINTH))):
        start = -H + (0.25 if course else 0)
        edges = sorted({x0, x1} | {start + k * 0.5 for k in range(-1, 6) if x0 < start + k * 0.5 < x1})
        for a, b in zip(edges, edges[1:]):
            a2 = a if a == x0 else a + 0.006
            b2 = b if b == x1 else b - 0.006
            p.span((a2, -0.15, za), (b2, 0.15, zb), "Charter navy")


def _bands(p: Piece, x0=-H, x1=H, plinth=True, string=True) -> None:
    if plinth:
        _plinth(p, x0, x1)
    if string:
        _run(p, STRING_PROFILE, x0, x1, PLINTH, "Charter white")
    p.span((x0, -0.135, FRIEZE), (x1, 0.135, CORNICE), "Charter navy")
    _run(p, CORNICE_PROFILE, x0, x1, CORNICE, "Charter white")
    p.span((x0, -0.16, COPING), (x1, 0.16, DECK), "Charter stone")


def _bands_low(p: Piece, x0=-H, x1=H, plinth=True, string=True) -> None:
    """The bands' stand-in: plain blocks and a simpler cornice."""
    with p.lowpoly():
        if plinth:
            p.span((x0, -0.15, 0), (x1, 0.15, PLINTH), "Charter navy")
        if string:
            p.span((x0, -0.165, PLINTH), (x1, 0.165, STRING), "Charter white")
        p.span((x0, -0.135, FRIEZE), (x1, 0.135, CORNICE), "Charter navy")
        _run(p, CORNICE_LOW, x0, x1, CORNICE, "Charter white")
        p.span((x0, -0.16, COPING), (x1, 0.16, DECK), "Charter stone")


def _wall_collider(p: Piece) -> None:
    p.collider((GRID + SEAM, 0.3, DECK), (0, 0, DECK / 2))


def _panel_lines(p: Piece, x0, x1, z0, z1) -> None:
    """A recessed panel picked out in a fine stone fillet, a centimetre proud
    of the render, on both faces."""
    for s in (-1, 1):
        y0, y1 = sorted((s * WALL, s * (WALL + 0.01)))
        for x in (x0, x1 - 0.02):
            p.span((x, y0, z0 + 0.02), (x + 0.02, y1, z1 - 0.02), "Charter stone")
        for z in (z0, z1 - 0.02):
            p.span((x0, y0, z), (x1, y1, z + 0.02), "Charter stone")


def charter_wall(style: Style) -> Piece:
    """2 m of company building: smooth white render between a rusticated
    navy plinth and a moulded cornice, a recessed panel picked out in stone."""
    p = Piece("charter_wall", "kit", "2 × 3.6 m white facade, 0.3 m thick: kickable", budget=500)
    _bands(p)
    p.span((-H, -WALL, STRING), (H, WALL, FRIEZE), "Charter white")
    _panel_lines(p, -0.72, 0.72, 0.85, FRIEZE - 0.2)
    _wall_collider(p)
    _bands_low(p)
    with p.lowpoly():
        p.span((-H, -WALL, STRING), (H, WALL, FRIEZE), "Charter white")
    return p


def charter_window(style: Style) -> Piece:
    """A Charter wall with a tall sash window: a navy frame of small panes
    set back in its reveal, a white architrave with a stone keystone, a stone
    sill and a navy hood over it."""
    p = Piece("charter_window", "kit", "2 × 3.6 m facade, 1 × 2.1 m window", budget=500)
    x0, x1, z0, z1 = -0.5, 0.5, 0.9, 3.0
    _bands(p)
    body = [((-H, STRING), (x0, FRIEZE)), ((x1, STRING), (H, FRIEZE)), ((x0, STRING), (x1, z0)), ((x0, z1), (x1, FRIEZE))]
    for (a, za), (b, zb) in body:
        p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
    _wall_collider(p)
    # The window, set back in the reveal: frame, meeting rail, glazing bars,
    # and the glass in its own plane between them.
    f, fy = 0.07, (0.0, 0.07)
    for xa, xb in ((x0, x0 + f), (x1 - f, x1)):
        p.span((xa, fy[0], z0), (xb, fy[1], z1), "Charter navy")
    mid = (z0 + z1) / 2
    for za, zb in ((z0, z0 + f), (z1 - f, z1), (mid - 0.035, mid + 0.035)):
        p.span((x0 + f, fy[0], za), (x1 - f, fy[1], zb), "Charter navy")
    p.span((x0 + 0.05, 0.031, z0 + 0.05), (x1 - 0.05, 0.039, z1 - 0.05), "Glass")
    for k in (1, 2, 4, 5):
        z = z0 + (z1 - z0) * k / 6
        p.span((x0 + f, 0.015, z - 0.012), (x1 - f, 0.055, z + 0.012), "Charter navy")
    p.span((-0.012, 0.015, z0 + f), (0.012, 0.055, z1 - f), "Charter navy")
    # The architrave both sides, the sill, keystone and hood on the street.
    for s in (-1, 1):
        ya, yb = sorted((s * WALL, s * (WALL + 0.03)))
        for xa, xb in ((x0 - 0.12, x0), (x1, x1 + 0.12)):
            p.span((xa, ya, z0), (xb, yb, z1 + 0.12), "Charter white")
        p.span((x0, ya, z1), (x1, yb, z1 + 0.12), "Charter white")
    p.span((x0 - 0.16, -0.22, z0 - 0.08), (x1 + 0.16, -WALL, z0), "Charter stone")
    p.prism([(-0.07, 0), (0.07, 0), (0.09, 0.16), (-0.09, 0.16)], 0.04, (0, -0.15, z1 - 0.02), "Charter stone")
    p.span((x0 - 0.18, -0.22, z1 + 0.12), (x1 + 0.18, -WALL, z1 + 0.2), "Charter navy")
    _bands_low(p)
    with p.lowpoly():
        for (a, za), (b, zb) in body:
            p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
        p.span((x0, 0.0, z0), (x1, 0.07, z1), "Charter navy")
        for s in (-1, 1):
            ya, yb = sorted((s * WALL, s * (WALL + 0.03)))
            for xa, xb in ((x0 - 0.12, x0), (x1, x1 + 0.12)):
                p.span((xa, ya, z0), (xb, yb, z1 + 0.12), "Charter white")
            p.span((x0, ya, z1), (x1, yb, z1 + 0.12), "Charter white")
        p.span((x0 - 0.16, -0.22, z0 - 0.08), (x1 + 0.16, -WALL, z0), "Charter stone")
        p.span((x0 - 0.18, -0.22, z1 + 0.12), (x1 + 0.18, -WALL, z1 + 0.2), "Charter navy")
    return p


def charter_door(style: Style) -> Piece:
    """A Charter wall with its front door: panelled double doors in navy with
    brass, a fanlight over them, pilasters and a pediment, and a stone step.
    It's kept locked (break-in jobs): the collider fills the doorway."""
    p = Piece("charter_door", "kit", "2 × 3.6 m facade, 1.2 × 2.4 m double door, a step", budget=500)
    x0, x1, top = -0.6, 0.6, 2.85
    _plinth(p, -H, x0)
    _plinth(p, x1, H)
    _run(p, STRING_PROFILE, -H, x0, PLINTH, "Charter white")
    _run(p, STRING_PROFILE, x1, H, PLINTH, "Charter white")
    _bands(p, plinth=False, string=False)
    body = [((-H, STRING), (x0, FRIEZE)), ((x1, STRING), (H, FRIEZE)), ((x0, top), (x1, FRIEZE))]
    for (a, za), (b, zb) in body:
        p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
    _wall_collider(p)
    # The doors: navy leaves, raised panels, brass handles and kick plates,
    # the meeting stiles, a knocker.
    p.span((x0, -0.03, 0), (x1, 0.03, 2.4), "Charter navy")
    with p.painted():
        for za, zb in ((0.02, 1.5), (1.7, 2.38)):
            p.span((-0.004, -0.036, za), (0.004, -0.031, zb), "Ink")
    for s in (-1, 1):
        cx = s * 0.3
        for za, zb in ((0.25, 0.95), (1.1, 2.25)):
            p.span((cx - 0.2, -0.045, za), (cx + 0.2, -0.03, zb), "Repaint navy")
            p.span((cx - 0.15, -0.055, za + 0.05), (cx + 0.15, -0.045, zb - 0.05), "Charter navy")
        p.span((cx - 0.26, -0.04, 0.02), (cx + 0.26, -0.03, 0.18), "Brass")
        p.lathe([(0.025, 0), (0.025, 0.01), (0.012, 0.02), (0.018, 0.05), (0, 0.06)], (s * 0.07, -0.03, 1.05), "Brass",
                rot=(90, 0, 0), segments=12)
    p.torus(0.06, 0.01, (0, -0.075, 1.55), "Brass", rot=(90, 0, 0), segments=16, sides=6)
    p.lathe([(0.03, 0), (0.03, 0.01), (0.015, 0.03), (0, 0.035)], (0, -0.03, 1.62), "Brass", rot=(90, 0, 0), segments=12)
    # The fanlight: a transom, glass in its own plane, radiating bars.
    p.span((x0, -0.04, 2.4), (x1, 0.04, 2.45), "Charter navy")
    p.span((x0 + 0.02, -0.004, 2.455), (x1 - 0.02, 0.004, top - 0.005), "Glass")
    for k in range(1, 6):
        x = x0 + (x1 - x0) * k / 6
        p.span((x - 0.012, -0.03, 2.45), (x + 0.012, 0.03, top), "Charter navy")
    # The surround: pilasters on stone bases, a navy entablature, a pediment
    # with a navy tympanum.
    for xa, xb in ((x0 - 0.16, x0 + 0.01), (x1 - 0.01, x1 + 0.16)):
        p.span((xa - 0.02, -0.19, 0.15), (xb + 0.02, -WALL, 0.45), "Charter stone")
        p.span((xa, -0.17, 0.45), (xb, -WALL, top), "Charter white")
        for k in range(3):
            fx = xa + 0.04 + k * 0.045
            p.span((fx, -0.178, 0.55), (fx + 0.012, -0.17, top - 0.1), "Charter stone")
    p.span((x0 - 0.22, -0.2, top), (x1 + 0.22, -WALL, top + 0.12), "Charter navy")
    p.prism([(-0.92, 0), (0.92, 0), (0, 0.3)], 0.08, (0, -0.16, top + 0.12), "Charter white")
    p.prism([(-0.7, 0.04), (0.7, 0.04), (0, 0.22)], 0.02, (0, -0.205, top + 0.12), "Charter navy")
    # The step.
    p.solid((-0.85, -0.62, 0), (0.85, -0.15, 0.15), "Charter stone")
    with p.lowpoly():
        p.span((-H, -0.15, 0), (x0, 0.15, PLINTH), "Charter navy")
        p.span((x1, -0.15, 0), (H, 0.15, PLINTH), "Charter navy")
        p.span((-H, -0.165, PLINTH), (x0, 0.165, STRING), "Charter white")
        p.span((x1, -0.165, PLINTH), (H, 0.165, STRING), "Charter white")
        for (a, za), (b, zb) in body:
            p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
        p.span((x0, -0.03, 0), (x1, 0.03, 2.4), "Charter navy")
        p.span((x0, -0.04, 2.4), (x1, 0.04, top), "Charter navy")
        for xa, xb in ((x0 - 0.16, x0 + 0.01), (x1 - 0.01, x1 + 0.16)):
            p.span((xa - 0.02, -0.19, 0.15), (xb + 0.02, -WALL, 0.45), "Charter stone")
            p.span((xa, -0.18, 0.45), (xb, -WALL, top), "Charter white")
        p.span((x0 - 0.22, -0.2, top), (x1 + 0.22, -WALL, top + 0.12), "Charter navy")
        p.prism([(-0.92, 0), (0.92, 0), (0, 0.3)], 0.1, (0, -0.17, top + 0.12), "Charter white")
        p.span((-0.85, -0.62, 0), (0.85, -0.15, 0.15), "Charter stone")
    _bands_low(p, plinth=False, string=False)
    return p


def charter_pillar(style: Style) -> Piece:
    """A corner pilaster for where two Charter walls meet: a rusticated navy
    base, a fluted white shaft, a stepped stone and white capital."""
    p = Piece("charter_pillar", "kit", "0.6 × 0.6 × 3.6 m corner pilaster", budget=500)
    p.span((-0.3, -0.3, 0), (0.3, 0.3, 0.29), "Charter navy")
    p.span((-0.295, -0.295, 0.29), (0.295, 0.295, 0.3), "Charter navy")
    p.span((-0.3, -0.3, 0.3), (0.3, 0.3, 0.6), "Charter navy")
    p.span((-0.31, -0.31, 0.6), (0.31, 0.31, 0.63), "Charter white")
    p.span((-0.29, -0.29, 0.63), (0.29, 0.29, 0.66), "Charter white")
    # The shaft: a core, and fillets standing proud of it on each face with
    # the flutes between them.
    p.span((-0.225, -0.225, 0.66), (0.225, 0.225, DECK - 0.35), "Charter white")
    for k in range(6):
        c = -0.21 + k * 0.084
        for s in (-1, 1):
            ya, yb = sorted((s * 0.225, s * 0.24))
            p.span((c - 0.012, ya, 0.66), (c + 0.012, yb, DECK - 0.35), "Charter white")
            p.span((ya, c - 0.012, 0.66), (yb, c + 0.012, DECK - 0.35), "Charter white")
    p.span((-0.28, -0.28, DECK - 0.35), (0.28, 0.28, DECK - 0.22), "Charter stone")
    p.span((-0.32, -0.32, DECK - 0.22), (0.32, 0.32, DECK - 0.12), "Charter white")
    p.span((-0.3, -0.3, DECK - 0.12), (0.3, 0.3, DECK), "Charter stone")
    p.collider((0.6, 0.6, DECK), (0, 0, DECK / 2))
    with p.lowpoly():
        p.span((-0.3, -0.3, 0), (0.3, 0.3, 0.6), "Charter navy")
        p.span((-0.31, -0.31, 0.6), (0.31, 0.31, 0.66), "Charter white")
        p.span((-0.24, -0.24, 0.66), (0.24, 0.24, DECK - 0.35), "Charter white")
        p.span((-0.28, -0.28, DECK - 0.35), (0.28, 0.28, DECK - 0.22), "Charter stone")
        p.span((-0.32, -0.32, DECK - 0.22), (0.32, 0.32, DECK - 0.12), "Charter white")
        p.span((-0.3, -0.3, DECK - 0.12), (0.3, 0.3, DECK), "Charter stone")
    return p


# The garden -------------------------------------------------------------------

def _clipped(p: Piece, lo, hi, seed: int, mat: str = "Clipped green", cell: float = 0.035, depth: float = 0.03) -> None:
    """A clipped block of greenery from lo to hi: a box split fine and its
    surface pushed in and out by small-scale noise, so its edges stay square
    while its faces are leafy, darker in the dips."""
    import bmesh
    from mathutils import Matrix, Vector, noise
    lo, hi = Vector(lo), Vector(hi)
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1, matrix=Matrix.Translation((lo + hi) / 2) @ Matrix.Diagonal((*(hi - lo), 1)))
    for _ in range(8):
        long = [e for e in bm.edges if e.calc_length() > cell]
        if not long:
            break
        bmesh.ops.subdivide_edges(bm, edges=long, cuts=1, use_grid_fill=True)
    bm.normal_update()
    # Leaf clumps: two scales of noise, pushed out along the surface; the
    # tops of the clumps, catching the light, are the lighter green.
    lift = {}
    for v in bm.verts:
        q = Vector((v.co.x + seed, v.co.y, v.co.z))
        n = noise.noise(q * 16) * 0.65 + noise.noise(q * 38) * 0.35
        lift[v] = n
        v.co += v.normal * depth * n
    i = p._slot(mat)
    light = p._slot("Clipped light")
    for f in bm.faces:
        f.smooth = True
        f.material_index = light if sum(lift[v] for v in f.verts) / len(f.verts) > 0.22 else i
    tmp = bpy.data.meshes.new("clipped")
    bm.to_mesh(tmp)
    bm.free()
    p.bm.from_mesh(tmp)
    bpy.data.meshes.remove(tmp)


def _topiary(p: Piece, r, at, size, mats=("Foliage", "Foliage light")) -> None:
    """A clipped shrub: a mass of overlapping leafy lumps (roughened leafy in
    the finish)."""
    x, y, z = at
    sx, sy, sz = size
    p.sphere(1, (x, y, z), mats[0], scale=(sx / 2, sy / 2, sz / 2), segments=9, rings=6)
    for i in range(5):
        a = math.tau * i / 5 + r.uniform(0, 1)
        # One green: lumps of two greens meeting at random would fight.
        p.sphere(1, (x + math.cos(a) * sx * 0.25, y + math.sin(a) * sy * 0.25, z + sz * r.uniform(0.0, 0.25)), mats[0],
                 scale=(sx * 0.28, sy * 0.28, sz * 0.3), segments=7, rings=5)


def glasshouse(style: Style) -> Piece:
    """A garden under glass, 4 × 3 m: a white-painted frame on a stone kerb,
    glazed walls and a pitched glass roof with ridge cresting and finials,
    raised beds of greens either side of the aisle and a citrus tree in a
    terracotta pot at the back. The door's at the front."""
    p = Piece("glasshouse", "prop", "4 × 3 m glasshouse, 3.3 m to the finials; door at the front", budget=45000)
    r = rng(style, p.name)
    w, d, kerb, eave, ridge = 2.0, 1.5, 0.4, 2.4, 3.2
    door = 0.5
    # The kerb, open at the door: a stone wall and a moulded white coping.
    for xa, xb in ((-w, -door), (door, w)):
        p.span((xa, -d, 0), (xb, -d + 0.15, kerb - 0.04), "Charter stone")
        p.span((xa - (0.01 if xa == -w else 0), -d - 0.01, kerb - 0.04), (xb + (0.01 if xb == w else 0), -d + 0.16, kerb), "Charter white")
    p.span((-w, d - 0.15, 0), (w, d, kerb - 0.04), "Charter stone")
    p.span((-w - 0.01, d - 0.16, kerb - 0.04), (w + 0.01, d + 0.01, kerb), "Charter white")
    for s in (-1, 1):
        x0, x1 = sorted((s * w, s * (w - 0.15)))
        p.span((x0, -d + 0.15, 0), (x1, d - 0.15, kerb - 0.04), "Charter stone")
        p.span((x0 - (0.01 if s < 0 else 0), -d + 0.16, kerb - 0.04), (x1 + (0.01 if s > 0 else 0), d - 0.16, kerb), "Charter white")
    # The glass, each sheet in its own plane inside the frame.
    g = 0.004
    for xa, xb in ((-w + 0.04, -door - 0.04), (door + 0.04, w - 0.04)):
        p.span((xa, -d + 0.075 - g, kerb), (xb, -d + 0.075 + g, eave), "Glass")
    p.span((-door + 0.04, -d + 0.075 - g, 2.15), (door - 0.04, -d + 0.075 + g, eave), "Glass")
    p.span((-w + 0.04, d - 0.075 - g, kerb), (w - 0.04, d - 0.075 + g, eave), "Glass")
    for s in (-1, 1):
        x = s * (w - 0.075)
        p.span((x - g, -d + 0.04, kerb), (x + g, d - 0.04, eave), "Glass")
        p.prism([(-d + 0.04, 0), (d - 0.04, 0), (0, ridge - eave - 0.05)], 2 * g, (x, 0, eave), "Glass", rot=(0, 0, 90))
    a = math.degrees(math.atan2(ridge - eave, d))
    slope = math.hypot(d, ridge - eave)
    for s in (-1, 1):
        p.box((2 * w - 0.08, slope - 0.06, 2 * g), (0, s * d / 2, (eave + ridge) / 2), "Glass", rot=(-s * a, 0, 0))
    # The white frame: posts, sills, eaves, ridge, rafters, glazing bars and
    # the door frame, all square-section and thicker than the glass.
    f = 0.035
    posts = (-w + 0.075, -1.0, -door, door, 1.0, w - 0.075)
    for x in posts:
        for y in (-d + 0.075, d - 0.075):
            p.span((x - f, y - f, kerb), (x + f, y + f, eave), "Charter white")
        for s in (-1, 1):
            p.box((0.05, slope + 0.05, 0.05), (x, s * d / 2, (eave + ridge) / 2 + 0.035), "Charter white", rot=(-s * a, 0, 0))
    for s in (-1, 1):
        for y in (-d / 2, 0.0, d / 2):
            p.span((s * (w - 0.075) - f, y - f, kerb), (s * (w - 0.075) + f, y + f, eave), "Charter white")
    for y in (-d + 0.075, d - 0.075):
        p.span((-w + 0.04, y - f, eave - f), (w - 0.04, y + f, eave + f), "Charter white")
        p.span((-w + 0.04, y - 0.02, 1.3 - 0.015), (w - 0.04, y + 0.02, 1.3 + 0.015), "Charter white")
    for s in (-1, 1):
        x = s * (w - 0.075)
        p.span((x - f, -d + 0.04, eave - f), (x + f, d - 0.04, eave + f), "Charter white")
        p.span((x - 0.02, -d + 0.04, 1.3 - 0.015), (x + 0.02, d - 0.04, 1.3 + 0.015), "Charter white")
    p.span((-w - 0.02, -0.045, ridge - 0.02), (w + 0.02, 0.045, ridge + 0.06), "Charter white")
    p.span((-door - f, -d + 0.035, 2.15 - f), (door + f, -d + 0.115, 2.15 + f), "Charter white")
    # The cresting along the ridge, and a finial at each end.
    for i in range(17):
        x = -w + 0.1 + i * (2 * w - 0.2) / 16
        p.prism([(-0.04, 0), (0.04, 0), (0.0, 0.09)], 0.012, (x, 0, ridge + 0.06), "Charter white")
    for x in (-w - 0.02, w + 0.02):
        p.lathe([(0.04, 0), (0.03, 0.05), (0.05, 0.1), (0.02, 0.16), (0.03, 0.2), (0, 0.26)], (x, 0, ridge + 0.06), "Charter white", segments=12)
    # The door, standing open on its hinges.
    p.span((door - 0.04, -d - 0.7, 0), (door, -d + 0.04, 2.1), "Charter white")
    p.span((door - 0.024, -d - 0.66, 0.05), (door - 0.016, -d, 2.05), "Glass")
    # The beds either side of the aisle, and what grows in them.
    for s in (-1, 1):
        x0, x1 = sorted((s * 0.45, s * (w - 0.2)))
        p.span((x0, -d + 0.25, 0), (x1, d - 0.25, 0.56), "Charter stone")
        p.span((x0 - 0.01, -d + 0.24, 0.56), (x1 + 0.01, d - 0.24, 0.6), "Charter white")
        p.span((x0 + 0.05, -d + 0.3, 0.6), (x1 - 0.05, d - 0.3, 0.62), "Soil")
        for i in range(5):
            y = -d + 0.5 + i * (2 * d - 1.0) / 4
            if r.random() < 0.5:
                _topiary(p, r, ((x0 + x1) / 2, y, 0.8), (0.5, 0.42, 0.38))
            else:
                for k in (-1, 1):
                    xx = (x0 + x1) / 2 + k * 0.25
                    for leaf in range(4):
                        t = math.tau * leaf / 4
                        p.box((0.06, 0.2, 0.012), (xx + math.cos(t) * 0.07, y + math.sin(t) * 0.07, 0.68), "Crop green",
                              rot=(25, 0, math.degrees(t) + 90))
                    p.sphere(0.07, (xx, y, 0.68), "Foliage light", segments=7, rings=5)
        p.collider((x1 - x0, 2 * d - 0.5, 0.6), ((x0 + x1) / 2, 0, 0.3))
    # The citrus tree at the back of the aisle, in its pot.
    p.lathe([(0.17, 0), (0.2, 0.03), (0.24, 0.4), (0.27, 0.42), (0.27, 0.47), (0.24, 0.48), (0, 0.46)], (0, d - 0.45, 0), "Terracotta", segments=20)
    p.cyl(0.035, 1.1, (0, d - 0.45, 1.0), "Wood dark", segments=8)
    _topiary(p, r, (0, d - 0.45, 1.8), (0.9, 0.8, 0.8))
    for i in range(9):
        t = math.tau * i / 9
        p.sphere(0.045, (math.cos(t) * 0.42, d - 0.45 + math.sin(t) * 0.36, 1.72 + 0.2 * math.sin(3 * t)), "Citrus", segments=6, rings=5)
    # Its walls, round the beds, open at the door.
    p.collider((2 * w, 0.15, eave), (0, d - 0.075, eave / 2))
    for s in (-1, 1):
        p.collider((0.15, 2 * d, eave), (s * (w - 0.075), 0, eave / 2))
        p.collider((w - door, 0.15, eave), (s * (w + door) / 2, -d + 0.075, eave / 2))
    return p


def planter(style: Style) -> Piece:
    """A planter along the Row: a white-painted plank box (Poly Haven's
    planter_box_02) and a clipped box shrub in it, 0.85 m to its top, so it
    can be vaulted."""
    p = Sourced("planter", "prop", "1.2 × 0.45 m planter, 0.72 m with the box shrub: vaultable", asset="planter_box_02",
                res="1k", length=1.2, recolour="Charter white")
    r = rng(style, p.name)
    p.span((-0.55, -0.18, 0.3), (0.55, 0.18, 0.4), "Soil")
    _clipped(p, (-0.54, -0.17, 0.38), (0.54, 0.17, 0.7), seed=r.randint(0, 99))
    p.collider((1.2, 0.45, 0.72), (0, 0, 0.36))
    return p


def hedge(style: Style) -> Piece:
    """2 m of clipped hedge, 0.95 m high and 0.7 m deep: low enough to vault.
    The Families' gardeners keep it square; the finish leaves it leafy."""
    p = Piece("hedge", "prop", "2 m hedge, 0.95 m high: vaultable")
    p.span((-H + 0.02, -0.33, 0), (H - 0.02, 0.33, 0.06), "Soil")
    _clipped(p, (-H + 0.025, -0.33, 0.04), (H - 0.025, 0.33, 0.93), seed=3, cell=0.055)
    p.collider((GRID + SEAM, 0.7, 0.95), (0, 0, 0.475))
    return p


def potted_tree(style: Style) -> Piece:
    """A potted tree for doorsteps and courtyards: Poly Haven's
    potted_plant_01, a leafy tree in a stone urn, grown to 2.2 m."""
    p = Sourced("potted_tree", "prop", "0.95 m urn, tree 2.2 m high", asset="potted_plant_01", res="1k", height=2.2)
    p.collider((0.75, 0.75, 0.66), (0, 0, 0.33))
    return p


# The guard and the gate ---------------------------------------------------------

def guard_booth(style: Style) -> Piece:
    """A private guard's booth: a navy plinth, white panelled walls, navy
    sash windows on three sides, a corniced flat roof with a lantern on it,
    the booth's number in brass, the Families' seal, and a door at the back.
    A stool and a shelf inside, glimpsed through the glass."""
    p = Piece("guard_booth", "prop", "1.4 × 1.4 m booth, 2.75 m high")
    w, sill, head, roof = 0.7, 1.0, 2.2, 2.3
    p.span((-w - 0.05, -w - 0.05, 0), (w + 0.05, w + 0.05, 0.15), "Charter navy")
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.span((sx * w - 0.07, sy * w - 0.07, 0.15), (sx * w + 0.07, sy * w + 0.07, roof), "Charter white")
    # The three glazed sides: a panelled wall to the sill, the sash window,
    # glass in its own plane, a head rail.
    for face in ("front", "left", "right"):
        def box(u0, u1, t0, t1, z0, z1, mat):
            """u along the face, t across it (outwards negative)."""
            if face == "front":
                p.span((u0, -w + t0, z0), (u1, -w + t1, z1), mat)
            elif face == "left":
                p.span((-w + t0, u0, z0), (-w + t1, u1, z1), mat)
            else:
                p.span((w - t1, u0, z0), (w - t0, u1, z1), mat)
        lo, hi = -w + 0.07, w - 0.07
        box(lo, hi, -0.02, 0.04, 0.15, sill, "Charter white")
        box(lo + 0.06, hi - 0.06, -0.035, -0.02, 0.25, sill - 0.08, "Repaint cream")
        box(lo - 0.02, hi + 0.02, -0.06, 0.05, sill, sill + 0.05, "Charter stone")
        box(lo, hi, -0.02, 0.04, head, roof, "Charter navy")
        box(lo + 0.005, hi - 0.005, 0.006, 0.014, sill + 0.055, head - 0.005, "Glass")
        for u in (lo + 0.02, (lo + hi) / 2 - 0.02, hi - 0.06):
            box(u, u + 0.04, -0.01, 0.03, sill + 0.05, head, "Charter navy")
        box(lo, hi, -0.01, 0.03, (sill + head) / 2 - 0.02, (sill + head) / 2 + 0.02, "Charter navy")
    # The back: a panelled door between white walls.
    p.span((-w + 0.07, w - 0.04, 0.15), (-0.35, w + 0.02, roof), "Charter white")
    p.span((0.35, w - 0.04, 0.15), (w - 0.07, w + 0.02, roof), "Charter white")
    p.span((-0.35, w - 0.04, 2.05), (0.35, w + 0.02, roof), "Charter white")
    p.span((-0.35, w - 0.03, 0.15), (0.35, w + 0.01, 2.05), "Charter navy")
    for za, zb in ((0.3, 1.0), (1.15, 1.9)):
        p.span((-0.27, w + 0.01, za), (0.27, w + 0.025, zb), "Repaint navy")
    p.lathe([(0.02, 0), (0.02, 0.01), (0.012, 0.02), (0.016, 0.04), (0, 0.05)], (0.25, w + 0.01, 1.05), "Brass", rot=(-90, 0, 0), segments=10)
    # The roof: an overhang, a moulded cornice, a lantern, the number and
    # the seal.
    p.span((-w - 0.1, -w - 0.1, roof), (w + 0.1, w + 0.1, roof + 0.1), "Charter navy")
    for s in (-1, 1):
        p.prism([(-0.02, 0), (0.12, 0), (0.12, 0.05), (0.06, 0.08), (-0.02, 0.08)], 2 * w + 0.36, (0, s * (w + 0.06), roof + 0.1), "Charter white",
                rot=(0, 0, 90 if s > 0 else -90))
        p.prism([(-0.02, 0), (0.12, 0), (0.12, 0.05), (0.06, 0.08), (-0.02, 0.08)], 2 * w + 0.36, (s * (w + 0.06), 0, roof + 0.1), "Charter white",
                rot=(0, 0, 180 if s > 0 else 0))
    p.span((-w - 0.15, -w - 0.15, roof + 0.18), (w + 0.15, w + 0.15, roof + 0.24), "Charter stone")
    p.lathe([(0.1, 0), (0.1, 0.03), (0.05, 0.08), (0.04, 0.16), (0, 0.16)], (0, 0, roof + 0.24), "Gunmetal", segments=12)
    p.cyl(0.08, 0.16, (0, 0, roof + 0.48), "Sodium lamp", segments=4, rot=(0, 0, 45))
    for i in range(4):
        a = math.tau * (i + 0.5) / 4
        p.cyl(0.008, 0.18, (math.cos(a) * 0.08, math.sin(a) * 0.08, roof + 0.48), "Gunmetal", segments=6)
    p.cyl(0.13, 0.08, (0, 0, roof + 0.6), "Gunmetal", segments=4, rot=(0, 0, 45), radius2=0.02)
    p.span((-0.12, -w - 0.155, roof + 0.02), (0.12, -w - 0.1, roof + 0.08), "Brass")
    p.stencil("02", (0, -w - 0.162, roof + 0.05), 0.04, "Charter navy")
    p.lathe([(0.12, 0), (0.12, 0.012), (0.1, 0.018), (0.08, 0.01), (0, 0.01)], (0, -w - 0.035, 0.6), "Brass", rot=(90, 0, 0), segments=24)
    p.cyl(0.075, 0.01, (0, -w - 0.048, 0.6), "Charter navy", rot=(90, 0, 0), segments=20)
    # Inside: a shelf, a ledger on it, a stool.
    p.span((-w + 0.08, -w + 0.06, 1.0), (w - 0.08, -w + 0.32, 1.04), "Wood")
    p.box((0.3, 0.22, 0.03), (0.2, -w + 0.2, 1.055), "Leather")
    p.lathe([(0.18, 0), (0.18, 0.012), (0.05, 0.03), (0.03, 0.05), (0, 0.05)], (0, 0.1, 0.15), "Hull dark", segments=14)
    p.cyl(0.025, 0.45, (0, 0.1, 0.42), "Steel", segments=8)
    p.cyl(0.17, 0.05, (0, 0.1, 0.67), "Leather green", segments=16)
    p.collider((2 * w + 0.1, 2 * w + 0.1, roof + 0.24), (0, 0, (roof + 0.24) / 2))
    return p


def iron_gate(style: Style) -> Piece:
    """A Charter gate: Poly Haven's large_iron_gate, a pair of scrolled iron
    leaves, drawn into a 2 m opening between rusticated stone piers with
    white caps and brass balls. It's shut."""
    p = Sourced("iron_gate", "prop", "2 m gate between piers, 2.5 m high", asset="large_iron_gate", res="1k",
                height=2.1, stretch=(0.616, 1, 1), budget=30000)
    pier = 0.35
    for s in (-1, 1):
        x0, x1 = sorted((s * H, s * (H - pier)))
        p.span((x0, -pier / 2, 0), (x1, pier / 2, 0.4), "Charter navy")
        p.span((x0 + 0.01, -pier / 2 + 0.01, 0.4), (x1 - 0.01, pier / 2 - 0.01, 2.2), "Charter stone")
        for z in (0.9, 1.5):
            p.span((x0, -pier / 2, z), (x1, pier / 2, z + 0.03), "Charter stone")
        p.span((x0 - 0.03, -pier / 2 - 0.03, 2.2), (x1 + 0.03, pier / 2 + 0.03, 2.3), "Charter white")
        p.lathe([(0.06, 0), (0.06, 0.03), (0.03, 0.05)], ((x0 + x1) / 2, 0, 2.3), "Charter white", segments=12)
        p.sphere(0.1, ((x0 + x1) / 2, 0, 2.43), "Brass", segments=14, rings=9)
    p.collider((GRID, pier, 2.3), (0, 0, 1.15))
    return p


def _spear(p: Piece, x, z0, z1, mat="Gunmetal") -> None:
    """A square railing bar, a collar near its top, a spear tip."""
    p.span((x - 0.012, -0.012, z0), (x + 0.012, 0.012, z1), mat)
    p.span((x - 0.018, -0.018, z1 - 0.13), (x + 0.018, 0.018, z1 - 0.11), mat)
    p.lathe([(0.02, 0), (0.028, 0.03), (0.012, 0.09), (0, 0.12)], (x, 0, z1), mat, segments=4, smooth=False, rot=(0, 0, 45))


def iron_fence(style: Style) -> Piece:
    """2 m of Charter fence: a low stone wall with a moulded white coping,
    iron railings to 1.9 m with collars and spear tips, and a slim pier with
    a white cap at its -X end. It's tiled by the dozen, so it's drawn as a
    low-poly stand-in with the detail baked on."""
    p = Piece("iron_fence", "prop", "2 m fence, 1.95 m high", budget=500)
    p.span((-H, -0.15, 0), (H, 0.15, 0.45), "Charter stone")
    _run(p, [(-0.17, 0), (0.17, 0), (0.17, 0.03), (0.15, 0.06), (0.12, 0.07), (-0.12, 0.07), (-0.15, 0.06), (-0.17, 0.03)],
         -H, H, 0.45, "Charter white")
    for z in (0.66, 1.66):
        p.span((-H + 0.22, -0.015, z - 0.025), (H, 0.015, z + 0.025), "Gunmetal")
    bars = [-H + 0.3 + k * (GRID - 0.36) / 13 for k in range(14)]
    for x in bars:
        _spear(p, x, 0.52, 1.78)
    p.span((-H, -0.15, 0.52), (-H + 0.22, 0.15, 1.82), "Charter stone")
    p.span((-H, -0.17, 1.82), (-H + 0.22, 0.17, 1.9), "Charter white")
    p.lathe([(0.05, 0), (0.05, 0.02), (0.02, 0.04)], (-H + 0.11, 0, 1.9), "Charter white", segments=10)
    p.sphere(0.045, (-H + 0.11, 0, 1.98), "Brass", segments=10, rings=6)
    p.collider((GRID + SEAM, 0.3, 1.9), (0, 0, 0.95))
    with p.lowpoly():
        p.span((-H, -0.15, 0), (H, 0.15, 0.45), "Charter stone")
        p.span((-H, -0.17, 0.45), (H, 0.17, 0.52), "Charter white")
        for z in (0.66, 1.66):
            p.span((-H + 0.22, -0.018, z - 0.025), (H, 0.018, z + 0.025), "Gunmetal")
        for x in bars:
            p.span((x - 0.018, -0.018, 0.52), (x + 0.018, 0.018, 1.78), "Gunmetal")
            p.prism([(-0.028, 0), (0.028, 0), (0, 0.12)], 0.03, (x, 0, 1.78), "Gunmetal")
        p.span((-H, -0.15, 0.45), (-H + 0.22, 0.15, 1.82), "Charter stone")
        p.span((-H, -0.17, 1.82), (-H + 0.22, 0.17, 1.9), "Charter white")
        p.span((-H + 0.07, -0.04, 1.9), (-H + 0.15, 0.04, 2.02), "Brass")
    return p


def street_lamp(style: Style) -> Piece:
    """A Charter Row street lamp: Poly Haven's street_lamp_01, a cast-iron
    post with its ladder bar and a four-sided lantern."""
    p = Sourced("street_lamp", "prop", "lamp post, 3.9 m high", asset="street_lamp_01", res="1k", height=3.9, budget=12000)
    p.collider((0.36, 0.36, 3.1), (0, 0, 1.55))
    return p


def stone_bench(style: Style) -> Piece:
    """A stone bench for the Row: a slab with a moulded edge and a navy
    inlay, on two scrolled supports."""
    p = Piece("stone_bench", "prop", "1.8 m stone bench, 0.45 m high: vaultable")
    _run(p, [(-0.25, 0), (0.25, 0), (0.25, 0.08), (0.24, 0.1), (0.22, 0.12), (-0.22, 0.12), (-0.24, 0.1), (-0.25, 0.08)],
         -0.9, 0.9, 0.33, "Charter stone")
    with p.painted():
        p.span((-0.88, -0.256, 0.36), (0.88, -0.251, 0.38), "Charter navy")
        p.span((-0.88, 0.251, 0.36), (0.88, 0.256, 0.38), "Charter navy")
    scroll = [(-0.2, 0), (0.2, 0), (0.2, 0.03), (0.15, 0.06), (0.13, 0.17), (0.16, 0.24), (0.21, 0.26), (0.21, 0.29),
              (-0.21, 0.29), (-0.21, 0.26), (-0.16, 0.24), (-0.13, 0.17), (-0.15, 0.06), (-0.2, 0.03)]
    for x in (-0.62, 0.62):
        p.prism(scroll, 0.22, (x, 0, 0.04), "Charter white", rot=(0, 0, 90))
        p.span((x - 0.13, -0.17, 0), (x + 0.13, 0.17, 0.04), "Charter navy")
    p.collider((1.8, 0.5, 0.45), (0, 0, 0.225))
    return p


# The office ---------------------------------------------------------------------

def office_desk(style: Style) -> Piece:
    """A Charter Family office desk: a polished top with a moulded edge on two
    panelled pedestals of drawers with brass pulls, a tooled leather inlay,
    the ledger open, an inkwell and pen, papers and a brass nameplate. What a
    theft contract comes for. The sitter is on the +Y side."""
    p = Piece("office_desk", "prop", "1.6 × 0.8 m pedestal desk, 0.76 m high")
    r = rng(style, p.name)
    top = 0.76
    _run(p, [(-0.42, 0), (0.42, 0), (0.42, 0.03), (0.41, 0.045), (0.4, 0.05), (-0.4, 0.05), (-0.41, 0.045), (-0.42, 0.03)],
         -0.82, 0.82, top - 0.05, "Wood dark")
    p.span((-0.7, -0.31, top), (0.7, 0.31, top + 0.006), "Leather green")
    with p.painted():
        for (xa, ya), (xb, yb) in (((-0.68, -0.296), (0.68, -0.29)), ((-0.68, 0.29), (0.68, 0.296)),
                                   ((-0.686, -0.29), (-0.68, 0.29)), ((0.68, -0.29), (0.686, 0.29))):
            p.span((xa, ya, top + 0.011), (xb, yb, top + 0.014), "Brass")
    for s in (-1, 1):
        x0, x1 = sorted((s * 0.78, s * 0.36))
        p.span((x0, -0.38, 0.06), (x1, 0.38, top - 0.05), "Wood")
        p.span((x0 - 0.02, -0.4, 0), (x1 + 0.02, 0.4, 0.06), "Wood dark")
        # The public side's raised panel, the sitter's three drawers.
        p.span((x0 + 0.04, -0.395, 0.12), (x1 - 0.04, -0.38, top - 0.11), "Wood dark")
        p.span((x0 + 0.08, -0.405, 0.16), (x1 - 0.08, -0.395, top - 0.15), "Wood")
        for i in range(3):
            z = 0.1 + i * 0.2
            p.span((x0 + 0.03, 0.38, z), (x1 - 0.03, 0.395, z + 0.17), "Wood")
            p.span((x0 + 0.05, 0.395, z + 0.02), (x1 - 0.05, 0.402, z + 0.15), "Wood dark")
            p.lathe([(0.02, 0), (0.02, 0.008), (0.01, 0.015), (0.016, 0.03), (0, 0.035)], ((x0 + x1) / 2, 0.402, z + 0.085), "Brass",
                    rot=(-90, 0, 0), segments=12)
    p.span((-0.36, -0.38, 0.3), (0.36, -0.34, top - 0.05), "Wood")
    for x in (-0.18, 0.18):
        p.span((x - 0.14, -0.395, 0.36), (x + 0.14, -0.38, top - 0.1), "Wood dark")
    p.collider((1.64, 0.84, top), (0, 0, top / 2))
    # The open ledger: two page blocks bowed from the spine on a leather cover.
    p.box((0.44, 0.33, 0.008), (0, 0.05, top + 0.01), "Leather")
    for s in (-1, 1):
        p.prism([(0, 0), (s * 0.2, 0.0), (s * 0.205, 0.014), (s * 0.1, 0.024), (0, 0.02)] if s > 0 else
                [(0, 0), (0, 0.02), (s * 0.1, 0.024), (s * 0.205, 0.014), (s * 0.2, 0.0)], 0.3, (0, 0.05, top + 0.014), "Paper")
        with p.painted():
            for k in range(8):
                p.box((0.15, 0.004, 0.003), (s * 0.1, -0.06 + k * 0.03, top + 0.043), "Ink")
    p.lathe([(0, 0), (0.045, 0), (0.05, 0.035), (0.03, 0.05), (0.018, 0.06), (0.02, 0.07), (0, 0.07)], (0.45, 0.12, top + 0.006), "Glass", segments=14)
    p.cyl(0.026, 0.01, (0.45, 0.12, top + 0.03), "Ink", segments=10)
    p.tube([(0.44, 0.12, top + 0.06), (0.36, 0.17, top + 0.18)], 0.004, "Ink", segments=6)
    p.cyl(0.006, 0.03, (0.43, 0.125, top + 0.05), "Brass", segments=6, rot=(0, 35, 0))
    for i in range(4):
        p.box((0.21, 0.29, 0.003), (-0.48, 0.12, top + 0.01 + i * 0.006), "Paper", rot=(0, 0, r.uniform(-5, 5)))
    p.span((-0.12, -0.27, top + 0.006), (0.12, -0.23, top + 0.05), "Brass")
    p.stencil("02", (0, -0.276, top + 0.028), 0.025, "Ink")
    return p


def office_chair(style: Style) -> Piece:
    """A Charter office chair: Poly Haven's dining_chair_02, a buttoned
    leather back on dark legs. It faces -Y, its back at +Y."""
    p = Sourced("office_chair", "prop", "0.45 × 0.6 m chair, 1 m high", asset="dining_chair_02", res="1k", height=1.0)
    p.collider((0.45, 0.58, 1.0), (0, 0, 0.5))
    return p


def safe(style: Style) -> Piece:
    """A Family's safe: navy steel with rounded corners, a recessed door on
    two barrel hinges, a brass combination dial with its ticks, a five-spoke
    handle wheel, a maker's plate, on four turned feet. Heavy enough that
    thieves crack it where it stands."""
    p = Piece("safe", "prop", "0.8 × 0.7 × 1.1 m safe")
    p.span((-0.4, -0.35, 0.07), (0.4, 0.35, 1.1), "Charter navy")
    for x in (-0.33, 0.33):
        for y in (-0.28, 0.28):
            p.lathe([(0.03, 0), (0.045, 0.02), (0.035, 0.05), (0.04, 0.07), (0, 0.07)], (x, y, 0), "Gunmetal", segments=12)
    # The door, set in a frame, a brass line round it.
    p.span((-0.34, -0.37, 0.12), (0.34, -0.35, 1.04), "Repaint navy")
    p.span((-0.3, -0.38, 0.16), (0.3, -0.37, 1.0), "Charter navy")
    with p.painted():
        for (xa, za), (xb, zb) in (((-0.29, 0.17), (0.29, 0.18)), ((-0.29, 0.98), (0.29, 0.99)),
                                   ((-0.29, 0.18), (-0.28, 0.98)), ((0.28, 0.18), (0.29, 0.98))):
            p.span((xa, -0.386, za), (xb, -0.382, zb), "Brass")
    for z in (0.3, 0.85):
        p.cyl(0.03, 0.16, (-0.37, -0.375, z), "Gunmetal", segments=12)
        for e in (-1, 1):
            p.sphere(0.03, (-0.37, -0.375, z + e * 0.08), "Gunmetal", segments=12, rings=6)
    # The dial: a turned brass bezel, the knurled knob, ticks round it.
    p.lathe([(0.1, 0), (0.1, 0.012), (0.085, 0.02), (0, 0.02)], (0.05, -0.38, 0.72), "Brass", rot=(90, 0, 0), segments=32)
    p.lathe([(0.055, 0), (0.06, 0.02), (0.055, 0.045), (0.03, 0.05), (0, 0.05)], (0.05, -0.4, 0.72), "Gunmetal", rot=(90, 0, 0), segments=24)
    with p.painted():
        for i in range(20):
            a = math.tau * i / 20
            p.box((0.004, 0.003, 0.012 if i % 5 else 0.02), (0.05 + math.cos(a) * 0.075, -0.405, 0.72 + math.sin(a) * 0.075), "Ink",
                  rot=(0, -math.degrees(a) + 90, 0))
    # The handle wheel.
    p.torus(0.12, 0.012, (0.05, -0.44, 0.42), "Brass", rot=(90, 0, 0), segments=24, sides=8)
    for i in range(5):
        a = math.radians(i * 72 + 90)
        p.cyl(0.008, 0.12, (0.05 + math.cos(a) * 0.06, -0.44, 0.42 + math.sin(a) * 0.06), "Brass", rot=(0, -math.degrees(a) + 90, 0), segments=8)
        p.sphere(0.018, (0.05 + math.cos(a) * 0.135, -0.44, 0.42 + math.sin(a) * 0.135), "Brass", segments=10, rings=6)
    p.lathe([(0.035, 0), (0.035, 0.03), (0.02, 0.06), (0, 0.065)], (0.05, -0.38, 0.42), "Brass", rot=(90, 0, 0), segments=14)
    p.span((-0.16, -0.39, 0.93), (0.16, -0.384, 0.97), "Brass")
    p.stencil("1", (0, -0.397, 0.95), 0.025, "Ink")
    p.collider((0.8, 0.7, 1.1), (0, 0, 0.55))
    return p


def bookshelf(style: Style) -> Piece:
    """A tall bookshelf: Poly Haven's wooden_bookshelf_worn, polished dark,
    drawn to 1.2 m wide, its shelves full of the Families' law books, charter
    copies and account books, banded and gilt, some lying flat."""
    p = Sourced("bookshelf", "prop", "1.2 × 0.4 × 2.2 m bookshelf", asset="wooden_bookshelf_worn", res="1k",
                height=2.2, stretch=(0.82, 0.645, 1.0), recolour="Wood dark")
    r = rng(style, p.name)
    shelves = [0.115, 0.418, 0.716, 1.024, 1.361, 1.773]
    gaps = [b - a - 0.04 for a, b in zip(shelves, shelves[1:] + [2.16])]
    colours = ["Leather", "Charter navy", "Fabric red", "Repaint green", "Leather green", "Wood", "Paper aged"]
    w = 0.52
    for z, gap in zip(shelves, gaps):
        x = -w
        while x < w - 0.06:
            if r.random() < 0.08 and x + 0.26 < w:
                # A few lying flat.
                for k in range(3):
                    p.span((x, -0.15, z + 0.002 + k * 0.041), (x + 0.24, 0.14, z + 0.002 + k * 0.041 + 0.038), r.choice(colours))
                x += 0.27
                continue
            t = r.uniform(0.045, 0.08)
            if x + t > w:
                break
            bh = min(gap - 0.02, r.uniform(0.2, 0.32))
            c = r.choice(colours)
            p.span((x, -0.15, z + 0.002), (x + t, 0.14, z + 0.002 + bh), c)
            p.span((x + 0.006, -0.145, z + 0.012), (x + t - 0.006, 0.142, z + bh - 0.008), "Paper aged")
            if r.random() < 0.5:
                for band in (0.12, 0.82):
                    p.span((x - 0.005, -0.156, z + bh * band), (x + t + 0.005, -0.14, z + bh * band + 0.015), "Brass")
            x += t + 0.003
    p.collider((1.2, 0.4, 2.2), (0, 0, 1.1))
    return p


def fountain(style: Style) -> Piece:
    """A small round fountain for a courtyard: a moulded stone basin of water
    on a navy plinth, a turned pedestal with a bowl that spills in four
    streams, a lion's-head of a finial in brass. Water this open is a Family
    showing off."""
    p = Piece("fountain", "prop", "2.6 m basin, 1.75 m high", budget=30000)
    p.cyl(1.28, 0.08, (0, 0, 0.04), "Charter navy", segments=48)
    p.lathe([(1.22, 0.08), (1.25, 0.1), (1.22, 0.35), (1.25, 0.38), (1.28, 0.42), (1.28, 0.46), (1.12, 0.48), (1.1, 0.44),
             (1.1, 0.12), (0, 0.12)], (0, 0, 0), "Charter stone", segments=48)
    p.cyl(1.105, 0.02, (0, 0, 0.39), "Water blue", segments=48)
    p.torus(1.2, 0.035, (0, 0, 0.47), "Charter white", segments=48, sides=8)
    p.lathe([(0.32, 0.4), (0.3, 0.46), (0.16, 0.56), (0.12, 0.6), (0.11, 0.95), (0.14, 1.0), (0.12, 1.05), (0.2, 1.1),
             (0.44, 1.15), (0.48, 1.2), (0.47, 1.25), (0, 1.22)], (0, 0, 0), "Charter stone", segments=32)
    p.cyl(0.43, 0.015, (0, 0, 1.235), "Water blue", segments=32)
    p.lathe([(0.1, 1.25), (0.07, 1.33), (0.1, 1.43), (0.06, 1.5), (0.04, 1.58), (0.06, 1.62), (0, 1.64)], (0, 0, 0), "Charter white", segments=16)
    p.sphere(0.06, (0, 0, 1.68), "Brass", segments=14, rings=9)
    for i in range(4):
        a = math.tau * (i + 0.5) / 4
        c, s = math.cos(a), math.sin(a)
        p.lathe([(0.03, 0), (0.025, 0.05), (0.035, 0.07), (0, 0.08)], (c * 0.47, s * 0.47, 1.18), "Brass", rot=(0, 70, math.degrees(a)), segments=8)
        p.tube([(c * 0.52, s * 0.52, 1.2), (c * 0.62, s * 0.62, 1.05), (c * 0.7, s * 0.7, 0.75), (c * 0.74, s * 0.74, 0.42)], 0.022,
               "Water blue", segments=8)
    for i in range(4):
        p.collider((2.5, 0.9, 0.48), (0, 0, 0.24), rot=(0, 0, i * 45))
    p.collider((0.6, 0.6, 1.25), (0, 0, 0.625))
    return p


# The roof and the storey above ----------------------------------------------
#
# An 8 × 8 m Charter building (Landfall's are) is closed by sixteen roof
# tiles on the 2 m grid, its edge by four parapets a side on the wall lines
# (centred on them, like the walls under them) and a corner pier at each
# corner, over the corner pilaster. The tiles run to the wall lines; the
# parapet's base covers their edge and the wall's coping, so neither shows.

ROOF = 0.3  # a roof tile's thickness, top at z = 0


def charter_roof(style: Style) -> Piece:
    """2×2 m of flat Charter roof, its top at z = 0: pale stone pavers on a
    bed of navy-painted slab, laid half a metre square with fine joints, a
    bronze drain in one in four; underneath, the white plaster ceiling of
    the room below. Walked on: one box collider. The game draws a slab."""
    p = Piece("charter_roof", "kit", "2 × 2 m flat roof, top at 0", budget=200)
    r = rng(style, p.name)
    p.collider((GRID, GRID, ROOF), (0, 0, -ROOF / 2))
    p.span((-H, -H, -ROOF + 0.02), (H, H, -0.03), "Charter navy")
    p.span((-H + 0.005, -H + 0.005, -ROOF), (H - 0.005, H - 0.005, -ROOF + 0.02), "Charter white")
    g = 0.006
    for i in range(4):
        for j in range(4):
            x0, y0 = -H + i * 0.5, -H + j * 0.5
            shade = r.choice(["Charter stone", "Charter stone", "Charter white"])
            p.span((x0 + g / 2, y0 + g / 2, -0.03), (x0 + 0.5 - g / 2, y0 + 0.5 - g / 2, 0), shade)
    if r.random() < 0.25:
        p.cyl(0.07, 0.012, (0.25, 0.25, 0.0), "Brass", segments=12)
        with p.painted():
            for k in range(4):
                p.span((0.25 - 0.05, 0.25 - 0.04 + k * 0.025, 0.011), (0.25 + 0.05, 0.25 - 0.03 + k * 0.025, 0.015), "Ink")
    with p.lowpoly():
        p.span((-H, -H, -ROOF), (H, H, 0), "Charter stone")
    return p


PARAPET = 0.9  # high enough to stop a fall, low enough to vault


def charter_parapet(style: Style) -> Piece:
    """2 m of parapet along a roof's edge, on a wall line and centred on it
    like the wall below: a stone base course over the wall's coping, white
    render with a panel picked out in stone on both faces, a moulded stone
    coping. 0.9 m high, so it's vaulted (if there's a drop beyond, the
    traversal won't take it)."""
    p = Piece("charter_parapet", "kit", "2 × 0.9 m parapet on a wall line", budget=500)
    p.span((-H, -0.17, 0), (H, 0.17, 0.06), "Charter stone")
    p.span((-H, -WALL, 0.06), (H, WALL, PARAPET - 0.15), "Charter white")
    _panel_lines(p, -0.8, 0.8, 0.14, PARAPET - 0.22)
    _run(p, [(-0.12, 0), (0.12, 0), (0.135, 0.015), (0.15, 0.03), (-0.15, 0.03), (-0.135, 0.015)],
         -H, H, PARAPET - 0.15, "Charter white")
    p.span((-H, -0.165, PARAPET - 0.12), (H, 0.165, PARAPET), "Charter stone")
    with p.painted():
        # The joints in the coping, a stone's length apart.
        for x in (-0.5, 0.0, 0.5):
            p.span((x - 0.003, -0.17, PARAPET - 0.12), (x + 0.003, -0.165, PARAPET - 0.005), "Concrete")
            p.span((x - 0.003, 0.165, PARAPET - 0.12), (x + 0.003, 0.17, PARAPET - 0.005), "Concrete")
    p.collider((GRID + SEAM, 0.3, PARAPET), (0, 0, PARAPET / 2))
    with p.lowpoly():
        p.span((-H, -0.17, 0), (H, 0.17, 0.06), "Charter stone")
        p.span((-H, -WALL, 0.06), (H, WALL, PARAPET - 0.12), "Charter white")
        p.span((-H, -0.165, PARAPET - 0.12), (H, 0.165, PARAPET), "Charter stone")
    return p


def _pyramid(p: Piece, at, half: float, height: float, mat: str) -> None:
    """A four-sided stone pyramid on a square of side 2 × half at at."""
    x, y, z = at
    base = [p.bm.verts.new((x + sx * half, y + sy * half, z)) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    tip = p.bm.verts.new((x, y, z + height))
    faces = [p.bm.faces.new(list(reversed(base)))]
    for i in range(4):
        faces.append(p.bm.faces.new((base[i], base[(i + 1) % 4], tip)))
    p._faces(faces, mat, smooth=False)


def charter_parapet_corner(style: Style) -> Piece:
    """The pier at a roof's corner, over the corner pilaster: a stone base
    course, a white shaft, a stepped stone cap and a stone pyramid on it.
    The parapets on both wall lines run into it."""
    p = Piece("charter_parapet_corner", "kit", "0.6 × 0.6 m corner pier, 1.3 m high", budget=500)
    p.span((-0.32, -0.32, 0), (0.32, 0.32, 0.06), "Charter stone")
    p.span((-0.27, -0.27, 0.06), (0.27, 0.27, PARAPET), "Charter white")
    for s in (-1, 1):
        for k in range(3):
            c = -0.12 + k * 0.12
            ya, yb = sorted((s * 0.27, s * 0.28))
            p.span((c - 0.02, ya, 0.2), (c + 0.02, yb, PARAPET - 0.12), "Charter stone")
            p.span((ya, c - 0.02, 0.2), (yb, c + 0.02, PARAPET - 0.12), "Charter stone")
    p.span((-0.31, -0.31, PARAPET), (0.31, 0.31, PARAPET + 0.06), "Charter stone")
    p.span((-0.34, -0.34, PARAPET + 0.06), (0.34, 0.34, PARAPET + 0.12), "Charter white")
    p.span((-0.3, -0.3, PARAPET + 0.12), (0.3, 0.3, PARAPET + 0.16), "Charter stone")
    _pyramid(p, (0, 0, PARAPET + 0.16), 0.22, 0.28, "Charter stone")
    p.collider((0.6, 0.6, PARAPET + 0.16), (0, 0, (PARAPET + 0.16) / 2))
    with p.lowpoly():
        p.span((-0.32, -0.32, 0), (0.32, 0.32, 0.06), "Charter stone")
        p.span((-0.28, -0.28, 0.06), (0.28, 0.28, PARAPET), "Charter white")
        p.span((-0.34, -0.34, PARAPET), (0.34, 0.34, PARAPET + 0.16), "Charter stone")
        _pyramid(p, (0, 0, PARAPET + 0.16), 0.22, 0.28, "Charter stone")
    return p


UPPER_BASE = 0.08  # the stone course an upper storey stands on


def _upper_bands(p: Piece, x0=-H, x1=H) -> None:
    """An upper storey's bands: a stone course at its foot over the coping of
    the storey below (which shows as a ledge between the two), and the frieze,
    cornice and coping at its top, as a ground-floor wall's."""
    p.span((x0, -0.14, 0), (x1, 0.14, UPPER_BASE), "Charter stone")
    _bands(p, x0, x1, plinth=False, string=False)


def _upper_bands_low(p: Piece) -> None:
    with p.lowpoly():
        p.span((-H, -0.14, 0), (H, 0.14, UPPER_BASE), "Charter stone")
    _bands_low(p, plinth=False, string=False)


def charter_wall_upper(style: Style) -> Piece:
    """2 m of a Charter building's upper storey: white render on a stone
    course over the storey below, a panel picked out in stone, and the
    frieze, cornice and coping at the top. Stacked on charter_wall (or any
    Charter wall) at z = 3.6."""
    p = Piece("charter_wall_upper", "kit", "2 × 3.6 m upper-storey facade, 0.3 m thick: kickable", budget=500)
    _upper_bands(p)
    p.span((-H, -WALL, UPPER_BASE), (H, WALL, FRIEZE), "Charter white")
    _panel_lines(p, -0.72, 0.72, 0.45, FRIEZE - 0.2)
    _wall_collider(p)
    _upper_bands_low(p)
    with p.lowpoly():
        p.span((-H, -WALL, UPPER_BASE), (H, WALL, FRIEZE), "Charter white")
    return p


def charter_window_upper(style: Style) -> Piece:
    """An upper storey's window: a tall sash of small panes set back in its
    reveal, the glass in its own plane, a white architrave on both faces, a
    stone sill and a navy hood, as charter_window's, between the stone
    course at the foot and the cornice at the top."""
    p = Piece("charter_window_upper", "kit", "2 × 3.6 m upper facade, 1 × 2 m window", budget=500)
    x0, x1, z0, z1 = -0.5, 0.5, 0.7, 2.7
    _upper_bands(p)
    body = [((-H, UPPER_BASE), (x0, FRIEZE)), ((x1, UPPER_BASE), (H, FRIEZE)), ((x0, UPPER_BASE), (x1, z0)),
            ((x0, z1), (x1, FRIEZE))]
    for (a, za), (b, zb) in body:
        p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
    _wall_collider(p)
    f, fy = 0.07, (0.0, 0.07)
    for xa, xb in ((x0, x0 + f), (x1 - f, x1)):
        p.span((xa, fy[0], z0), (xb, fy[1], z1), "Charter navy")
    mid = (z0 + z1) / 2
    for za, zb in ((z0, z0 + f), (z1 - f, z1), (mid - 0.035, mid + 0.035)):
        p.span((x0 + f, fy[0], za), (x1 - f, fy[1], zb), "Charter navy")
    p.span((x0 + 0.05, 0.031, z0 + 0.05), (x1 - 0.05, 0.039, z1 - 0.05), "Glass")
    for k in (1, 2, 4, 5):
        z = z0 + (z1 - z0) * k / 6
        p.span((x0 + f, 0.015, z - 0.012), (x1 - f, 0.055, z + 0.012), "Charter navy")
    p.span((-0.012, 0.015, z0 + f), (0.012, 0.055, z1 - f), "Charter navy")
    for s in (-1, 1):
        ya, yb = sorted((s * WALL, s * (WALL + 0.03)))
        for xa, xb in ((x0 - 0.12, x0), (x1, x1 + 0.12)):
            p.span((xa, ya, z0), (xb, yb, z1 + 0.12), "Charter white")
        p.span((x0, ya, z1), (x1, yb, z1 + 0.12), "Charter white")
    p.span((x0 - 0.16, -0.22, z0 - 0.08), (x1 + 0.16, -WALL, z0), "Charter stone")
    p.prism([(-0.07, 0), (0.07, 0), (0.09, 0.16), (-0.09, 0.16)], 0.04, (0, -0.15, z1 - 0.02), "Charter stone")
    p.span((x0 - 0.18, -0.22, z1 + 0.12), (x1 + 0.18, -WALL, z1 + 0.2), "Charter navy")
    _upper_bands_low(p)
    with p.lowpoly():
        for (a, za), (b, zb) in body:
            p.span((a, -WALL, za), (b, WALL, zb), "Charter white")
        p.span((x0, 0.0, z0), (x1, 0.07, z1), "Charter navy")
        for s in (-1, 1):
            ya, yb = sorted((s * WALL, s * (WALL + 0.03)))
            for xa, xb in ((x0 - 0.12, x0), (x1, x1 + 0.12)):
                p.span((xa, ya, z0), (xb, yb, z1 + 0.12), "Charter white")
            p.span((x0, ya, z1), (x1, yb, z1 + 0.12), "Charter white")
        p.span((x0 - 0.16, -0.22, z0 - 0.08), (x1 + 0.16, -WALL, z0), "Charter stone")
        p.span((x0 - 0.18, -0.22, z1 + 0.12), (x1 + 0.18, -WALL, z1 + 0.2), "Charter navy")
    return p


def charter_roof_unit(style: Style) -> Piece:
    """An air-handling unit on a Charter roof, as clean as the street below
    it: a white casing with navy corner posts on a stone curb, louvred
    sides, a fan behind a ringed guard on top, a service door with a brass
    handle and the company's plate, its duct turning down into the roof."""
    p = Piece("charter_roof_unit", "prop", "1.5 × 1 × 1.2 m rooftop air unit", budget=20000)
    w, d, h = 0.65, 0.42, 0.95
    p.span((-w - 0.1, -d - 0.1, 0), (w + 0.1, d + 0.1, 0.12), "Charter stone")
    b = 0.12
    p.span((-w, -d, b), (w, d, h), "Charter white")
    for sx in (-1, 1):
        for sy in (-1, 1):
            x0, x1 = sorted((sx * w, sx * (w - 0.05)))
            y0, y1 = sorted((sy * d, sy * (d - 0.05)))
            p.span((x0 - (0.006 if sx < 0 else 0), y0 - (0.006 if sy < 0 else 0), b - 0.01),
                   (x1 + (0.006 if sx > 0 else 0), y1 + (0.006 if sy > 0 else 0), h + 0.02), "Charter navy")
    p.span((-w - 0.006, -d - 0.006, h), (w + 0.006, d + 0.006, h + 0.03), "Charter navy")
    # Louvres on the sides and the back.
    for side in (-1, 1):
        x = side * (w + 0.005)
        for k in range(9):
            z = b + 0.1 + k * 0.075
            p.box((0.012, 2 * d - 0.14, 0.05), (x, 0, z), "Charter white", rot=(0, side * 35, 0))
    for k in range(9):
        z = b + 0.1 + k * 0.075
        p.box((2 * w - 0.14, 0.012, 0.05), (0, d + 0.005, z), "Charter white", rot=(35, 0, 0))
    # The service door on the front, its hinges, handle and plate.
    p.span((-0.45, -d - 0.012, b + 0.08), (0.2, -d, h - 0.08), "Charter white")
    for z in (b + 0.18, h - 0.18):
        p.cyl(0.012, 0.06, (-0.45, -d - 0.012, z), "Charter navy", segments=6)
    p.span((0.12, -d - 0.03, 0.5), (0.15, -d - 0.012, 0.62), "Brass")
    p.span((0.28, -d - 0.006, 0.6), (0.5, -d, 0.74), "Brass")
    with p.painted():
        p.lettering("AHU 2", (0.39, -d - 0.013, 0.67), 0.035, "Charter navy")
    # The fan on top: a ringed guard over the blades, plain.
    fz = h + 0.03
    p.cyl(0.36, 0.08, (0, 0, fz + 0.04), "Charter navy", segments=16)
    with p.plain():
        for k in range(5):
            a = 72 * k
            p.box((0.3, 0.09, 0.01), (math.cos(math.radians(a)) * 0.17, math.sin(math.radians(a)) * 0.17, fz + 0.07),
                  "Hull dark", rot=(20, 0, a))
        for rad in (0.12, 0.22, 0.32):
            p.torus(rad, 0.006, (0, 0, fz + 0.1), "Steel", segments=16, sides=3)
        for k in range(4):
            p.box((0.66, 0.012, 0.012), (0, 0, fz + 0.1), "Steel", rot=(0, 0, 45 * k))
    p.cyl(0.05, 0.04, (0, 0, fz + 0.1), "Hull dark", segments=8)
    # The duct: out of the back, turning down into the roof.
    p.tube([(0.35, d, 0.55), (0.35, d + 0.3, 0.55), (0.35, d + 0.3, 0.012)], 0.1, "Steel", segments=10)
    p.cyl(0.14, 0.03, (0.35, d + 0.3, 0.015), "Charter navy", segments=12)
    p.collider((2 * w + 0.2, 2 * d + 0.2, h + 0.1), (0, 0, (h + 0.1) / 2))
    return p


PIECES = [charter_wall, charter_window, charter_door, charter_pillar, glasshouse, planter, hedge, potted_tree, guard_booth,
          iron_gate, iron_fence, street_lamp, stone_bench, office_desk, office_chair, safe, bookshelf, fountain,
          charter_roof, charter_parapet, charter_parapet_corner, charter_wall_upper, charter_window_upper,
          charter_roof_unit]
