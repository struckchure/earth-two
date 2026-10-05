"""The dome line round Landfall, and the South gate through it. The domes
are geodesic frames of salvaged strut and glass, fogged at the edges, and
inside them the light is filtered and warm. The South gate is the only way
in from the Fringe, so it's where gate checks happen and where smuggling
contracts are won or lost (docs/settlement.md).

Each piece stands on its origin in metres. A piece of the dome line has its
front (-Y) looking out at the Fringe and its back (+Y) inside the dome; the
glass leans in. The dome's walls and frames are laid by the hundred, so the
game draws them as a few slabs with their detail baked on (kit's
Piece.lowpoly); the gate and its checkpoint carry theirs as geometry."""
import math

from mathutils import Vector

from kit import DECK, PALETTE, Piece, Style, rng

CATEGORY = "Domes and gate"

PALETTE.setdefault("Glass fog", (0.66, 0.68, 0.64))

# How far the dome's lower wall leans in from upright, and its height.
LEAN = 15.0
FOOT = 0.6
WALL = 5.0


def _slant(s: float, x: float = 0.0, off: float = 0.0):
    """A point s metres up the leaning glass from the footing, x along it and
    off out of its plane (towards the inside)."""
    a = math.radians(LEAN)
    return (x, s * math.sin(a) + off * math.cos(a), FOOT + s * math.cos(a) - off * math.sin(a))


def _beam(p: Piece, a, b, w: float, h: float, mat: str) -> None:
    """A square-section member from a to b, w across and h deep."""
    a, b = Vector(a), Vector(b)
    d = b - a
    q = d.normalized().to_track_quat("Z", "Y").to_euler()
    p.box((w, h, d.length), (a + b) / 2, mat, rot=tuple(math.degrees(r) for r in q))


def _hazard(p: Piece, lo, hi, along: str = "z", n: int = 0) -> None:
    """Yellow and dark in turn along a band of paint (a box a few
    millimetres proud of its face): the gate's warning stripes."""
    lo, hi = list(lo), list(hi)
    i = "xyz".index(along)
    length = hi[i] - lo[i]
    n = n or max(2, round(length / 0.3))
    with p.painted():
        for k in range(n):
            a, b = list(lo), list(hi)
            a[i] = lo[i] + length * k / n
            b[i] = lo[i] + length * (k + 1) / n
            p.span(a, b, "Hazard yellow" if k % 2 == 0 else "Hull dark")


def _bolts(p: Piece, points, radius: float, depth: float, mat: str, rot=(0, 0, 0)) -> None:
    """Bolt heads, hexagonal (in the detailed build), at points."""
    for pt in points:
        p.cyl(radius, depth, pt, mat, rot=rot, segments=2.4)


def _pane(p: Piece, s0: float, s1: float, x0: float, x1: float, fog: float) -> None:
    """One pane of the leaning wall from s0 to s1 up it and x0 to x1 along
    it: clear glass in the middle, fogged glass round its edges where the
    seal's aged, as neighbouring regions of one sheet (never two layered)."""
    rot = (-LEAN, 0, 0)
    t = 0.03
    xa, xb, sa, sb = x0 + fog, x1 - fog, s0 + fog, s1 - fog
    p.box((xb - xa, t, sb - sa), _slant((sa + sb) / 2, (xa + xb) / 2), "Glass", rot=rot)
    for (u0, u1, v0, v1) in ((x0, x1, s0, sa), (x0, x1, sb, s1), (x0, xa, sa, sb), (xb, x1, sa, sb)):
        p.box((u1 - u0, t, v1 - v0), _slant((v0 + v1) / 2, (u0 + u1) / 2), "Glass fog", rot=rot)


def dome_wall(style: Style) -> Piece:
    """4 m of the dome's lower wall: a cast footing of red-soil concrete, its
    top lip chamfered and a steel sill on it, and a glazed wall rising off it
    and leaning in, mullions and transoms with bolted pressure caps, its
    panes fogged round their seals. Red dust banks up against it outside.
    The game draws a footing and a slab of glass (some 60 triangles) with
    all that baked on; the end mullions are halves, so walls laid side by
    side share one."""
    p = Piece("dome_wall", "kit", "4 m of dome wall, 5.1 m high, leaning in to +Y")
    p.budget = 500
    w = 2.0
    footing = [(-0.4, 0.0), (0.4, 0.0), (0.4, FOOT - 0.12), (0.32, FOOT - 0.02), (-0.32, FOOT - 0.02), (-0.4, FOOT - 0.12)]
    p.prism(footing, 2 * w, (0, 0, 0), "Red concrete", rot=(0, 0, 90))
    # Parts that run the full width stop 5 mm short of its ends, so nothing
    # meets the footing's end faces in one plane.
    e = 0.005
    p.span((-w + e, -0.3, FOOT - 0.02), (w - e, 0.3, FOOT), "Steel")
    p.collider((2 * w, 0.8, FOOT), (0, 0, FOOT / 2))
    length = (WALL - FOOT) / math.cos(math.radians(LEAN))
    rot = (-LEAN, 0, 0)
    # Mullions (halves at the ends), transoms, and their pressure caps
    # bolted on the outside.
    for x0, x1 in ((-w + 2 * e, -w + 0.07), (-0.07, 0.07), (w - 0.07, w - 2 * e)):
        cx = (x0 + x1) / 2
        p.box((x1 - x0, 0.18, length), _slant(length / 2, cx), "Hull dark", rot=rot)
        cap0, cap1 = max(x0 + e, cx - 0.04), min(x1 - e, cx + 0.04)
        p.box((cap1 - cap0, 0.03, length - 0.06), _slant(length / 2, (cap0 + cap1) / 2, -0.105), "Hull alloy", rot=rot)
        for k in range(9):
            s = 0.2 + k * (length - 0.4) / 8
            p.cyl(0.012, 0.02, _slant(s, (cap0 + cap1) / 2, -0.125), "Steel", rot=(90 - LEAN, 0, 0), segments=2.4)
    rows = [0.0, length * 0.36, length * 0.7, length]
    for s in rows:
        p.box((2 * w - 4 * e, 0.2, 0.14), _slant(s, 0), "Hull dark", rot=rot)
        p.box((2 * w - 6 * e, 0.03, 0.08), _slant(s, 0, -0.115), "Hull alloy", rot=rot)
    # The panes, each glass fogged round its edges.
    r = rng(style, p.name)
    for i in range(3):
        s0, s1 = rows[i] + 0.07, rows[i + 1] - 0.07
        for x0, x1 in ((-w + 0.07, -0.07), (0.07, w - 0.07)):
            _pane(p, s0, s1, x0, x1, fog=r.uniform(0.14, 0.24))
    p.collider((2 * w, 0.2, length), _slant(length / 2), rot=rot)
    # Drain weeps and form-tie marks along the footing, a sector number, and
    # red dust banked against it outside.
    with p.painted():
        for x in (-1.5, -0.5, 0.5, 1.5):
            p.span((x - 0.05, -0.406, 0.14), (x + 0.05, -0.402, 0.2), "Hull dark")
        for x in (-1.0, 1.0):
            for z in (0.18, 0.38):
                p.cyl(0.02, 0.004, (x, -0.404, z), "Rock dark", rot=(90, 0, 0), segments=4)
        p.stencil("4", (0, -0.405, 0.32), 0.16, "Stencil white")
    drift = [(-w + e, 0.0), (w - e, 0.0), (w - e, 0.06), (w * 0.55, 0.2), (0.0, 0.14), (-w * 0.5, 0.22), (-w + e, 0.08)]
    p.prism(drift, 0.3, (0, -0.56, 0), "Dust red")
    with p.lowpoly():
        p.prism(footing, 2 * w, (0, 0, 0), "Red concrete", rot=(0, 0, 90))
        p.box((2 * w, 0.24, length), _slant(length / 2, 0, -0.02), "Glass", rot=rot)
        p.prism([(-w, 0.0), (w, 0.0), (w, 0.08), (0.0, 0.2), (-w, 0.1)], 0.3, (0, -0.56, 0), "Dust red")
    return p


def dome_frame(style: Style) -> Piece:
    """A big triangle of the geodesic dome above the lower wall: struts of
    channel bolted to cast hubs, split into four panes, each glass fogged
    round its seal, one pane long since replaced with riveted sheet. Stood
    upright on its base strut; the dome is built from these, turned and
    tilted. It has no collider: it's for up out of reach."""
    p = Piece("dome_frame", "kit", "6 m triangle of dome frame, 5.5 m high; no collider")
    p.budget = 800
    a, b, c = (-3.0, 0.0), (3.0, 0.0), (0.0, 5.2)
    mid = lambda u, v: ((u[0] + v[0]) / 2, (u[1] + v[1]) / 2)  # noqa: E731
    ab, bc, ca = mid(a, b), mid(b, c), mid(c, a)
    hubs = [a, b, c, ab, bc, ca]
    tris = [(a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca)]
    struts = [(a, ab), (ab, b), (b, bc), (bc, c), (c, ca), (ca, a), (ab, bc), (bc, ca), (ca, ab)]
    lift = 0.17

    def strut(u, v, low=False):
        dx, dz = v[0] - u[0], v[1] - u[1]
        length = math.hypot(dx, dz) - 0.3
        phi = -math.degrees(math.atan2(dz, dx))
        at = ((u[0] + v[0]) / 2, 0, (u[1] + v[1]) / 2 + lift)
        if low:
            p.box((length, 0.2, 0.14), at, "Hull alloy", rot=(0, phi, 0))
            return
        # A channel: the web, and two flanges standing out front and back.
        p.box((length, 0.16, 0.06), at, "Hull alloy", rot=(0, phi, 0))
        for k in (-1, 1):
            p.box((length, 0.02, 0.14), (at[0], k * 0.09, at[2]), "Hull alloy", rot=(0, phi, 0))

    def wound(tri):
        # Wound anticlockwise in (x, z), as the prism wants.
        if (tri[1][0] - tri[0][0]) * (tri[2][1] - tri[0][1]) - (tri[1][1] - tri[0][1]) * (tri[2][0] - tri[0][0]) < 0:
            return (tri[0], tri[2], tri[1])
        return tri

    def inset(tri, d):
        cx = sum(t[0] for t in tri) / 3
        cz = sum(t[1] for t in tri) / 3
        out = []
        for x, z in tri:
            vx, vz = x - cx, z - cz
            k = max(0.0, 1 - d / math.hypot(vx, vz))
            out.append((cx + vx * k, cz + vz * k + lift))
        return out

    for u, v in struts:
        strut(u, v)
    for x, z in hubs:
        p.cyl(0.19, 0.26, (x, 0, z + lift), "Hull dark", rot=(90, 0, 0), segments=5)
        p.cyl(0.09, 0.3, (x, 0, z + lift), "Steel", rot=(90, 0, 0), segments=4)
        _bolts(p, [(x + 0.135 * math.cos(t), -0.135, z + lift + 0.135 * math.sin(t)) for t in (k * math.tau / 6 + 0.26 for k in range(6))],
               0.018, 0.02, "Steel", rot=(90, 0, 0))
    r = rng(style, p.name)
    patched = r.randrange(4)
    for i, tri in enumerate(tris):
        tri = wound(tri)
        outer = inset(tri, 0.16)
        if i == patched:
            # Sheet riveted in where the glass was.
            p.prism(outer, 0.03, (0, 0, 0), "Repaint oxide")
            cx = sum(q[0] for q in outer) / 3
            cz = sum(q[1] for q in outer) / 3
            for k, q in enumerate(outer):
                nq = outer[(k + 1) % 3]
                for f in (0.2, 0.4, 0.6, 0.8):
                    x = q[0] + (nq[0] - q[0]) * f
                    z = q[1] + (nq[1] - q[1]) * f
                    x, z = x + (cx - x) * 0.08, z + (cz - z) * 0.08
                    p.cyl(0.012, 0.016, (x, -0.022, z), "Hull dark", rot=(90, 0, 0), segments=2.4)
            continue
        # Clear glass inside, a band of fogged glass round it, edge to edge.
        inner = inset(tri, 0.16 + r.uniform(0.3, 0.45))
        p.prism(inner, 0.03, (0, 0, 0), "Glass")
        for k in range(3):
            o0, o1 = outer[k], outer[(k + 1) % 3]
            i0, i1 = inner[k], inner[(k + 1) % 3]
            p.prism([o0, o1, i1, i0], 0.03, (0, 0, 0), "Glass fog")
    with p.lowpoly():
        for u, v in struts:
            strut(u, v, low=True)
        for x, z in hubs:
            p.cyl(0.19, 0.3, (x, 0, z + lift), "Hull dark", rot=(90, 0, 0), segments=2.4)
        for tri in tris:
            p.prism(inset(wound(tri), 0.12), 0.04, (0, 0, 0), "Glass")
    return p


def dome_strut_anchor(style: Style) -> Piece:
    """The foot of the dome: a cast pier with a bolted steel shoe and
    gussets, where three struts meet a hub and come down to the ground, red
    dust banked against its weather side."""
    p = Piece("dome_strut_anchor", "kit", "1.2 m pier, struts to 2.2 m; vaultable")
    p.budget = 600
    pier = [(-0.6, 0.0), (0.6, 0.0), (0.6, 0.42), (0.52, 0.5), (-0.52, 0.5), (-0.6, 0.42)]
    p.prism(pier, 1.2, (0, 0, 0), "Red concrete", rot=(0, 0, 90))
    p.span((-0.45, -0.45, 0.5), (0.45, 0.45, 0.56), "Steel")
    _bolts(p, [(sx * 0.36, sy * 0.36, 0.58) for sx in (-1, 1) for sy in (-1, 1)], 0.035, 0.05, "Hull dark")
    p.collider((1.2, 1.2, 0.5), (0, 0, 0.25))
    hub = (0, 0, 0.92)
    for k in range(4):
        a = k * math.pi / 2 + math.pi / 4
        p.prism([(-0.02, 0.0), (0.02, 0.0), (0.02, 0.06), (-0.02, 0.3)], 0.3, (0.12 * math.cos(a), 0.12 * math.sin(a), 0.56), "Hull dark",
                rot=(0, 0, math.degrees(a)))
    p.cyl(0.17, 0.32, (0, 0, 0.72), "Hull dark", segments=5)
    p.sphere(0.2, hub, "Hull dark", segments=6, rings=4)
    ends = ((-1.1, 0.5, 2.1), (1.1, 0.5, 2.1), (0, 1.0, 2.2))
    for end in ends:
        p.tube([hub, end], 0.08, "Hull alloy", segments=4)
        d = (Vector(end) - Vector(hub)).normalized()
        q = d.to_track_quat("Z", "Y").to_euler()
        rot = tuple(math.degrees(v) for v in q)
        p.cyl(0.12, 0.04, end, "Hull dark", rot=rot, segments=4)
        p.cyl(0.11, 0.04, tuple(Vector(hub) + d * 0.26), "Hull dark", rot=rot, segments=4)
    with p.painted():
        p.stencil("17", (0, -0.605, 0.25), 0.16, "Stencil white")
    drift = [(-0.6, 0.0), (0.6, 0.0), (0.5, 0.12), (-0.3, 0.2)]
    p.prism(drift, 0.3, (0, -0.76, 0), "Dust red")
    with p.lowpoly():
        p.prism(pier, 1.2, (0, 0, 0), "Red concrete", rot=(0, 0, 90))
        p.span((-0.45, -0.45, 0.5), (0.45, 0.45, 0.6), "Steel")
        p.cyl(0.19, 0.42, (0, 0, 0.8), "Hull dark", segments=2.4)
        for end in ends:
            p.tube([hub, end], 0.1, "Hull alloy", segments=2.4)
        p.prism(drift, 0.3, (0, -0.76, 0), "Dust red")
    return p


def _floodlight(p: Piece, at, aim: float = 30.0) -> None:
    """A gate floodlight on an arm out of the wall at at: a ribbed housing
    and its sodium lens, aimed aim degrees down towards -Y."""
    x, y, z = at
    p.span((x - 0.05, y - 0.32, z - 0.05), (x + 0.05, y, z + 0.05), "Hull dark")
    c = Vector((x, y - 0.42, z - 0.05))
    a = math.radians(aim)
    d = Vector((0, -math.cos(a), -math.sin(a)))  # where it shines
    rot = tuple(math.degrees(v) for v in d.to_track_quat("Z", "Y").to_euler())
    p.lathe([(0.0, -0.12), (0.18, -0.12), (0.22, 0.0), (0.22, 0.1), (0.0, 0.1)], tuple(c), "Crew grey", rot=rot, segments=8)
    p.cyl(0.19, 0.02, tuple(c + d * 0.105), "Sodium lamp", rot=rot, segments=8)
    for k in range(4):
        p.torus(0.07 + k * 0.035, 0.007, tuple(c - d * 0.125), "Hull dark", rot=rot, segments=8, sides=2.4)


def south_gate(style: Style) -> Piece:
    """The South gate: the one door between Landfall and the Fringe. Two
    cast pillars and a lintel of red-soil concrete faced with riveted steel,
    a 5 × 4 m opening, its two sliding leaves run back on their track and
    left there for years, hazard paint, slit windows and a speaker grille,
    floodlights and a camera on the approach, the gate's number and a
    Registrar seal. Caravans come in here, and so does everything else."""
    p = Piece("south_gate", "kit", "8 m gate, 6.6 m high, 5 × 4 m opening, 4.5 m clear between its leaves")
    p.budget = 60000
    r = rng(style, p.name)
    half, open_w, open_h, top = 4.0, 2.5, 4.0, 6.5
    # The pillars, faced with steel plates riveted on, a slit window with its
    # frame, a speaker grille, conduit running up them.
    for sx in (-1, 1):
        x0, x1 = sorted((sx * open_w, sx * half))
        p.span((x0, -0.5, 0), (x1, 0.5, top), "Red concrete")
        p.collider((x1 - x0, 1.0, top), ((x0 + x1) / 2, 0, top / 2))
        for z0, z1 in ((0.3, 1.5), (1.52, 2.9), (2.92, open_h)):
            p.span((x0 + 0.12, -0.525, z0), (x1 - 0.12, -0.5, z1), "Hull alloy")
            _bolts(p, [(xx, -0.53, zz) for xx in (x0 + 0.2, (x0 + x1) / 2, x1 - 0.2) for zz in (z0 + 0.08, z1 - 0.08)],
                   0.02, 0.02, "Steel", rot=(90, 0, 0))
        wx = sx * 3.25
        p.span((wx - 0.38, -0.56, 2.12), (wx + 0.38, -0.525, 2.58), "Hull dark")
        p.span((wx - 0.3, -0.575, 2.2), (wx + 0.3, -0.56, 2.5), "Glass")
        p.span((wx - 0.18, -0.545, 1.62), (wx + 0.18, -0.525, 1.88), "Hull dark")
        for k in range(5):
            p.span((wx - 0.15, -0.56, 1.65 + k * 0.045), (wx + 0.15, -0.545, 1.672 + k * 0.045), "Grating")
        # Conduit up the pillar's outer face, and a junction box.
        cx = sx * (half - 0.12)
        p.tube([(cx, -0.56, 0.1), (cx, -0.56, top - 0.4), (cx, -0.3, top - 0.25)], 0.035, "Hull dark", segments=6)
        bx = cx - sx * 0.3
        p.span((bx - 0.15, -0.58, 4.65), (bx + 0.15, -0.5, 5.05), "Crew grey")
        p.span((bx - 0.13, -0.595, 4.67), (bx + 0.13, -0.58, 5.03), "Crew grey")
        # A beacon on top of each pillar.
        p.cyl(0.12, 0.08, (sx * 3.25, 0, top + 0.18), "Hull dark", segments=8)
        p.lathe([(0.1, 0.0), (0.1, 0.12), (0.06, 0.2), (0.0, 0.21)], (sx * 3.25, 0, top + 0.22), "Sodium lamp", segments=8)
        # Hazard paint down the opening's edges, front and back.
        edge = sx * open_w
        for y0, y1 in ((-0.508, -0.504), (0.504, 0.508)):
            _hazard(p, (edge - (0.3 if sx > 0 else 0), y0, 0.02), (edge + (0 if sx > 0 else 0.3), y1, open_h - 0.02), n=12)
    # The lintel, its cap, its plate and number, and the Registrar's seal.
    p.span((-open_w, -0.5, open_h), (open_w, 0.5, top), "Red concrete")
    p.collider((2 * open_w, 1.0, top - open_h), (0, 0, (open_h + top) / 2))
    p.span((-half - 0.12, -0.62, top), (half + 0.12, 0.62, top + 0.14), "Concrete")
    _hazard(p, (-open_w + 0.02, -0.508, open_h + 0.02), (open_w - 0.02, -0.504, open_h + 0.32), along="x", n=16)
    p.span((-1.7, -0.58, 4.95), (0.3, -0.5, 6.15), "Hull dark")
    _bolts(p, [(x, -0.59, z) for x in (-1.6, 0.2) for z in (5.05, 6.05)], 0.025, 0.02, "Steel", rot=(90, 0, 0))
    with p.painted():
        p.stencil("01", (-0.7, -0.587, 5.55), 0.7, "Stencil white")
    p.lathe([(0.0, 0.0), (0.36, 0.0), (0.36, 0.04), (0.3, 0.06), (0.28, 0.1), (0.18, 0.1), (0.16, 0.13), (0.0, 0.13)],
            (1.0, -0.5, 5.55), "Brass", rot=(90, 0, 0), segments=16)
    # The track along the front on its brackets, the guide rail on the
    # ground, and the leaves hanging off the track on their trolleys.
    p.span((-half - 0.95, -0.88, open_h + 0.05), (half + 0.95, -0.56, open_h + 0.3), "Hull dark")
    for x in (-4.4, -3.0, -1.0, 1.0, 3.0, 4.4):
        p.span((x - 0.08, -0.56, open_h + 0.3), (x + 0.08, -0.5, open_h + 0.6), "Hull dark")
    p.span((-half - 0.95, -0.86, 0), (half + 0.95, -0.54, 0.05), "Steel")
    leaves = [(-half - 0.9, -open_w + 0.25), (open_w - 0.25, half + 0.9)]
    for x0, x1 in leaves:
        # Both run back almost clear of the opening, 4.5 m between them.
        p.span((x0, -0.8, 0.06), (x1, -0.6, open_h), "Hull alloy")
        p.collider((x1 - x0, 0.2, open_h - 0.06), ((x0 + x1) / 2, -0.7, open_h / 2 + 0.03))
        for x in (x0, x1 - 0.12):
            p.span((x, -0.83, 0.08), (x + 0.12, -0.8, open_h - 0.02), "Crew grey")
        for i in range(5):
            z = 0.45 + i * 0.8
            p.span((x0 + 0.13, -0.825, z), (x1 - 0.13, -0.8, z + 0.12), "Crew grey")
        for x in (x0 + 0.4, x1 - 0.4):
            p.cyl(0.11, 0.14, (x, -0.72, open_h + 0.18), "Steel", rot=(90, 0, 0), segments=8)
            p.span((x - 0.04, -0.75, open_h - 0.05), (x + 0.04, -0.69, open_h + 0.1), "Hull dark")
        _hazard(p, (x0 + 0.13, -0.808, 0.1), (x1 - 0.13, -0.804, 0.4), along="x", n=max(3, round((x1 - x0) / 0.3)))
    # A plate welded over a breach on the right leaf, riveted round.
    patch = r.choice(["Repaint oxide", "Repaint teal", "Container blue"])
    p.span((2.9, -0.815, 1.4), (3.8, -0.8, 2.0), patch)
    _bolts(p, [(x, -0.82, z) for x in (2.98, 3.35, 3.72) for z in (1.47, 1.93)], 0.015, 0.012, "Hull dark", rot=(90, 0, 0))
    # Floodlights on the lintel, angled down on the approach, and a camera.
    for x in (-3.0, 3.0):
        _floodlight(p, (x, -0.5, top - 0.25))
    p.span((1.9, -0.6, top - 0.6), (2.0, -0.5, top - 0.2), "Hull dark")
    p.box((0.16, 0.42, 0.16), (1.95, -0.86, top - 0.3), "Crew grey", rot=(-15, 0, 0))
    p.cyl(0.05, 0.03, (1.95, -1.1, top - 0.36), "Screen", rot=(75, 0, 0), segments=8)
    # Red dust banked against the pillars' feet, both sides.
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.prism([(-0.75, 0.0), (0.75, 0.0), (0.5, 0.12), (-0.2, 0.18)], 0.35, (sx * 3.25, sy * 1.05, 0), "Dust red")
    return p


def boom_barrier(style: Style) -> Piece:
    """The gate check's boom: a motor cabinet with louvres and a domed
    drive, a striped arm of square tube with reflectors, a counterweight
    behind, and the forked rest it comes down onto. Its underside is 0.95 m
    up: too low to walk under, low enough to slide."""
    p = Piece("boom_barrier", "prop", "4.5 m boom, underside 0.95 m: slide under")
    p.budget = 10000
    # The cabinet on its plinth, a door, louvres, the drive dome.
    p.span((-0.3, -0.3, 0), (0.3, 0.3, 0.06), "Concrete")
    p.span((-0.25, -0.25, 0.06), (0.25, 0.25, 1.06), "Crew grey")
    p.span((-0.27, -0.27, 1.06), (0.27, 0.27, 1.1), "Hazard yellow")
    p.collider((0.5, 0.5, 1.1), (0, 0, 0.55))
    for k in range(6):
        p.box((0.3, 0.02, 0.02), (0, -0.258, 0.3 + k * 0.05), "Hull dark", rot=(35, 0, 0))
    p.span((-0.2, -0.262, 0.62), (0.2, -0.25, 0.9), "Crew grey")
    p.cyl(0.012, 0.03, (0.16, -0.27, 0.76), "Steel", rot=(90, 0, 0), segments=2.4)
    p.lathe([(0.12, 0.0), (0.12, 0.04), (0.09, 0.08), (0.0, 0.1)], (0, 0.25, 1.0), "Hull dark", rot=(-90, 0, 0), segments=10)
    _bolts(p, [(sx * 0.27, sy * 0.27, 0.07) for sx in (-1, 1) for sy in (-1, 1)], 0.018, 0.02, "Steel")
    # The arm: square tube from the hub out to its tip, its paint, the
    # counterweight behind the post.
    p.span((0.0, -0.37, 0.95), (4.25, -0.27, 1.05), "Stencil white")
    p.collider((4.0, 0.1, 0.1), (2.25, -0.32, 1.0))
    p.span((-0.75, -0.38, 0.9), (-0.28, -0.26, 1.1), "Hull dark")
    p.cyl(0.07, 0.14, (0, -0.32, 1.0), "Steel", rot=(90, 0, 0), segments=8)
    with p.painted():
        for k in range(4):
            x0 = 0.3 + k * 1.0
            for y in (-0.376, -0.264):
                p.span((x0, y - 0.002, 0.955), (x0 + 0.5, y + 0.002, 1.045), "Medical red")
            p.span((x0, -0.36, 1.055), (x0 + 0.5, -0.28, 1.059), "Medical red")
        for x in (1.05, 2.05, 3.05, 4.05):
            p.cyl(0.02, 0.004, (x, -0.377, 1.0), "Sodium lamp", rot=(90, 0, 0), segments=4)
    # The rest it comes down onto: a post, a fork, a foot.
    p.span((4.15, -0.38, 0.03), (4.25, -0.26, 0.9), "Crew grey")
    p.prism([(-0.13, 0.0), (0.13, 0.0), (0.13, 0.14), (0.07, 0.14), (0.05, 0.05), (-0.05, 0.05), (-0.07, 0.14), (-0.13, 0.14)], 0.12,
            (4.2, -0.32, 0.9), "Hull dark", rot=(0, 0, 90))
    p.span((4.05, -0.47, 0), (4.35, -0.17, 0.03), "Concrete")
    return p


def checkpoint_booth(style: Style) -> Piece:
    """The gate check, where the Crew and the Registrars look at your papers
    and your mask: a booth of cast walls and a steel frame, glazed down one
    side and across the front either side of a hatch with a sliding tray, a
    desk with a terminal and the stamp inside, a roof with a hazard edge, an
    air conditioner, a camera, a beacon, the Crew's band and the
    Registrars' seal."""
    p = Piece("checkpoint_booth", "prop", "2.4 × 2 × 2.9 m booth, hatch to -Y")
    p.budget = 20000
    w, d, sill, head, roof = 1.2, 1.0, 1.05, 2.2, 2.6
    p.collider((2 * w, 2 * d, roof), (0, 0, roof / 2))
    # A plinth, the lower walls, corner posts, the band above the windows.
    p.span((-w - 0.03, -d - 0.03, 0), (w + 0.03, d + 0.03, 0.08), "Concrete")
    # Four cast walls, 12 cm thick, to the sill; inside, the floor's the plinth.
    t = 0.12
    p.span((-w, -d, 0.08), (w, -d + t, sill), "Red concrete")
    p.span((-w, d - t, 0.08), (w, d, sill), "Red concrete")
    p.span((-w, -d + t, 0.08), (-w + t, d - t, sill), "Red concrete")
    p.span((w - t, -d + t, 0.08), (w, d - t, sill), "Red concrete")
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.span((sx * w - (0.12 if sx > 0 else 0), sy * d - (0.12 if sy > 0 else 0), sill),
                   (sx * w + (0 if sx > 0 else 0.12), sy * d + (0 if sy > 0 else 0.12), head), "Crew grey")
    p.span((-w, -d, head), (w, d, roof), "Crew grey")
    p.span((-w - 0.2, -d - 0.32, roof), (w + 0.2, d + 0.2, roof + 0.15), "Hull dark")
    _hazard(p, (-w - 0.2, -d - 0.326, roof + 0.01), (w + 0.2, -d - 0.322, roof + 0.14), along="x", n=12)
    # The back and the -X side are solid, the door in the back.
    p.span((-w + 0.12, d - 0.08, sill), (w - 0.12, d, head), "Crew grey")
    p.span((-w, -d + 0.12, sill), (-w + 0.08, d - 0.12, head), "Crew grey")
    p.span((-0.42, d, 0.08), (0.42, d + 0.04, 2.1), "Hull dark")
    p.span((-0.36, d + 0.04, 1.4), (0.36, d + 0.055, 1.9), "Glass")
    p.cyl(0.02, 0.06, (0.3, d + 0.07, 1.1), "Steel", rot=(90, 0, 0), segments=2.4)
    # The +X side's glass in its frame, and the front: a hatch between two
    # panes, the tray under it.
    p.span((w - 0.08, -d + 0.12, sill), (w, d - 0.12, sill + 0.06), "Crew grey")
    p.span((w - 0.08, -d + 0.12, head - 0.06), (w, d - 0.12, head), "Crew grey")
    p.span((w - 0.05, -d + 0.12, sill + 0.06), (w - 0.03, d - 0.12, head - 0.06), "Glass")
    for x0, x1 in ((-w + 0.12, -0.5), (0.5, w - 0.12)):
        p.span((x0, -d, sill), (x1, -d + 0.08, sill + 0.06), "Crew grey")
        p.span((x0, -d, head - 0.06), (x1, -d + 0.08, head), "Crew grey")
        p.span((x0, -d + 0.03, sill + 0.06), (x1, -d + 0.05, head - 0.06), "Glass")
    for x in (-0.45, 0.45):
        p.span((x - 0.05, -d, sill), (x + 0.05, -d + 0.1, head), "Crew grey")
    p.span((-0.4, -d - 0.32, sill - 0.05), (0.4, -d + 0.1, sill - 0.01), "Hull alloy")
    p.span((-0.4, -d + 0.04, 1.7), (0.4, -d + 0.06, head), "Glass")
    # Inside: the desk, the terminal, a stamp, a stool.
    p.span((-w + 0.1, -d + 0.1, 0.86), (w - 0.1, -d + 0.65, 0.9), "Wood")
    p.box((0.56, 0.12, 0.42), (-0.5, -d + 0.52, 1.15), "Hull dark", rot=(-15, 0, 0))
    p.box((0.48, 0.02, 0.34), (-0.5, -d + 0.45, 1.16), "Status blue", rot=(-15, 0, 0))
    p.lathe([(0.0, 0.0), (0.045, 0.0), (0.045, 0.02), (0.02, 0.04), (0.02, 0.1), (0.035, 0.12), (0.0, 0.14)], (0.3, -d + 0.35, 0.9), "Wood dark", segments=8)
    p.cyl(0.05, 0.5, (0, 0.25, 0.33), "Hull dark", segments=6)
    p.cyl(0.2, 0.05, (0, 0.25, 0.605), "Rubber", segments=10)
    # An air conditioner on the -X side, its fan grille.
    p.span((-w - 0.35, -0.4, 1.3), (-w, 0.4, 1.85), "Charter white")
    p.cyl(0.21, 0.02, (-w - 0.36, 0, 1.575), "Grating", rot=(0, 90, 0), segments=10)
    p.cyl(0.05, 0.03, (-w - 0.375, 0, 1.575), "Hull dark", rot=(0, 90, 0), segments=6)
    p.tube([(-w - 0.2, 0.4, 1.4), (-w - 0.2, 0.5, 1.4), (-w - 0.2, 0.5, 0.1)], 0.02, "Rubber", segments=4)
    # A camera on the roof's corner, and the beacon.
    p.span((w - 0.1, -d - 0.25, roof + 0.15), (w, -d - 0.15, roof + 0.4), "Hull dark")
    p.box((0.14, 0.34, 0.14), (w - 0.05, -d - 0.4, roof + 0.35), "Crew grey", rot=(-20, 0, 0))
    p.cyl(0.045, 0.02, (w - 0.05, -d - 0.575, roof + 0.29), "Screen", rot=(70, 0, 0), segments=8)
    p.cyl(0.08, 0.05, (-w + 0.2, 0.0, roof + 0.175), "Hull dark", segments=8)
    p.lathe([(0.07, 0.0), (0.07, 0.08), (0.04, 0.13), (0.0, 0.14)], (-w + 0.2, 0.0, roof + 0.2), "Status blue", segments=8)
    with p.painted():
        p.span((-w + 0.01, -d - 0.008, 0.72), (w - 0.01, -d - 0.004, 0.92), "Crew orange")
        p.stencil("02", (-0.7, -d - 0.008, 0.45), 0.24, "Stencil white")
    p.lathe([(0.0, 0.0), (0.14, 0.0), (0.14, 0.02), (0.11, 0.04), (0.0, 0.045)], (0.8, -d, 0.45), "Brass", rot=(90, 0, 0), segments=14)
    return p


def scanner_arch(style: Style) -> Piece:
    """A walk-through scanner at the gate: two posts and a header in a
    steel case, lit strips down their inner faces, a floor plate with ramped
    edges, and the operator's console beside it on its stand, a cable
    between. It finds unfiled weapons, mostly."""
    p = Piece("scanner_arch", "prop", "1.8 m arch, 1 m clear, 2.6 m high")
    p.budget = 10000
    for sx in (-1, 1):
        x0, x1 = sorted((sx * 0.5, sx * 0.9))
        p.span((x0, -0.3, 0.04), (x1, 0.3, 2.3), "Crew grey")
        for y0, y1 in ((-0.312, -0.3), (0.3, 0.312)):
            p.span((x0 + 0.02, y0, 0.12), (x1 - 0.02, y1, 2.22), "Hull alloy")
        edge = sx * 0.5
        p.span((edge - (0.012 if sx > 0 else 0.0), -0.15, 0.2), (edge + (0.0 if sx > 0 else 0.012), 0.15, 2.1), "Status blue")
        p.collider((0.4, 0.6, 2.3), ((x0 + x1) / 2, 0, 1.15))
        p.span((x0 - 0.02, -0.32, 0.0), (x1 + 0.02, 0.32, 0.04), "Hull dark")
    p.span((-0.92, -0.32, 2.3), (0.92, 0.32, 2.6), "Hull dark")
    _hazard(p, (-0.92, -0.328, 2.36), (0.92, -0.324, 2.54), along="x", n=8)
    p.collider((1.8, 0.6, 0.3), (0, 0, 2.45))
    # The floor plate, ramped at its ends.
    p.span((-0.48, -0.38, 0), (0.48, 0.38, 0.03), "Grating")
    for sy in (-1, 1):
        p.prism([(sy * 0.38, 0.0), (sy * 0.5, 0.0), (sy * 0.38, 0.03)], 0.96, (0, 0, 0), "Hull dark", rot=(0, 0, 90))
    p.box((0.12, 0.06, 0.06), (0, -0.35, 2.24), "Status blue")
    # The console on its stand to the left, its screen and buttons.
    p.span((-1.52, -0.22, 0), (-1.18, 0.12, 0.05), "Hull dark")
    p.span((-1.42, -0.12, 0.05), (-1.28, 0.02, 0.96), "Hull dark")
    p.box((0.6, 0.45, 0.08), (-1.35, -0.1, 1.02), "Crew grey", rot=(25, 0, 0))
    with p.painted():
        p.box((0.4, 0.27, 0.004), (-1.35, -0.12, 1.068), "Status blue", rot=(25, 0, 0))
    for k in range(3):
        p.cyl(0.015, 0.02, (-1.55 + k * 0.05, -0.27, 1.005), "Fabric red" if k == 0 else "Foliage light", rot=(25, 0, 0), segments=2.4)
    p.collider((0.34, 0.34, 1.0), (-1.35, -0.05, 0.5))
    p.cable((-1.18, 0.0, 0.03), (-0.92, 0.0, 0.03), 0.02, "Rubber", sag=-0.02, segments=3, steps=6)
    return p


def airlock_door(style: Style) -> Piece:
    """A pedestrian airlock through a wall: a 1.2 × 2.2 m opening in a deep
    frame with gussets in its top corners and hazard stripes, its door slid
    most of the way into the wall (a wired window, a wheel), a riveted plate
    either side, the conduit over it and the lamp that says it's cycled.
    The game draws the wall, frame and door as slabs, with that baked on."""
    p = Piece("airlock_door", "kit", "2 m wall, 3.6 m high, 1.2 × 2.2 m opening, 0.9 m clear")
    p.budget = 500
    w, half, top, d = 1.0, 0.6, 2.2, 0.25
    p.solid((-w, -d, 0), (-half, d, DECK), "Hull alloy")
    p.solid((half, -d, 0), (w, d, DECK), "Hull alloy")
    p.solid((-half, -d, top), (half, d, DECK), "Hull alloy")
    f = 0.16
    for side in (-1, 1):
        y0, y1 = sorted((side * d, side * (d + 0.08)))
        for x in (-half - f, half):
            p.span((x, y0, 0), (x + f, y1, top + f), "Crew grey")
        p.span((-half - f, y0, top), (half + f, y1, top + f), "Crew grey")
        for x in (-half, half):
            p.prism([(0, 0), (0.18, 0.18), (0, 0.18)] if x < 0 else [(0, 0.18), (-0.18, 0.18), (0, 0)], 0.08,
                    (x, (y0 + y1) / 2, top - 0.18), "Crew grey")
        face = y1 if side > 0 else y0
        with p.painted():
            for x in (-half - f, half):
                for i in range(7):
                    p.box((f - 0.02, 0.004, 0.12), (x + f / 2, face + side * 0.006, 0.15 + i * 0.3), "Hazard yellow", rot=(0, 30, 0))
        # Plates riveted on the wall's faces either side.
        for x0, x1 in ((-w + 0.05, -half - f - 0.03), (half + f + 0.03, w - 0.05)):
            fy = side * (d + 0.015)
            p.span((x0, min(side * d, fy), 0.15), (x1, max(side * d, fy), DECK - 0.15), "Hull alloy")
            _bolts(p, [(xx, side * (d + 0.02), zz) for xx in (x0 + 0.06, x1 - 0.06) for zz in (0.3, 1.2, 2.1, 3.0, DECK - 0.3)],
                   0.015, 0.012, "Steel", rot=(90, 0, 0))
    # The door, slid into the wall from +X: 0.3 m of it still across the gap.
    # Its wired window sits in it edge to edge, not over it.
    dx0, dx1 = 0.3, w - 0.05
    p.span((dx0, -0.05, 0.02), (dx1, 0.05, 1.4), "Repaint teal")
    p.span((dx0, -0.05, 1.75), (dx1, 0.05, top - 0.02), "Repaint teal")
    p.span((dx0, -0.05, 1.4), (0.32, 0.05, 1.75), "Repaint teal")
    p.span((0.56, -0.05, 1.4), (dx1, 0.05, 1.75), "Repaint teal")
    p.span((0.32, -0.05, 1.4), (0.56, 0.05, 1.75), "Glass")
    p.collider((w - 0.35, 0.1, top - 0.04), ((dx0 + dx1) / 2, 0, top / 2))
    p.torus(0.12, 0.02, (0.45, -0.08, 1.05), "Fabric red", rot=(90, 0, 0), segments=10, sides=2.4)
    for k in range(3):
        a = k * math.tau / 3
        _beam(p, (0.45, -0.08, 1.05), (0.45 + 0.12 * math.cos(a), -0.08, 1.05 + 0.12 * math.sin(a)), 0.015, 0.015, "Fabric red")
    p.cyl(0.03, 0.06, (0.45, -0.07, 1.05), "Hull dark", rot=(90, 0, 0), segments=2.4)
    # The conduit over the frame, the cycle lamp, and the stencil.
    p.tube([(-w, -d - 0.11, top + 0.75), (w, -d - 0.11, top + 0.75)], 0.035, "Hull dark", segments=4)
    p.span((-0.12, -d - 0.16, top + 0.32), (0.12, -d - 0.02, top + 0.48), "Hull dark")
    p.span((-0.09, -d - 0.175, top + 0.35), (0.09, -d - 0.16, top + 0.45), "Status blue")
    with p.painted():
        p.stencil("02", (0, -d - 0.022, top + 1.05), 0.32, "Stencil white")
    with p.lowpoly():
        p.span((-w, -d - 0.02, 0), (-half - f, d + 0.02, DECK), "Hull alloy")
        p.span((half + f, -d - 0.02, 0), (w, d + 0.02, DECK), "Hull alloy")
        p.span((-half - f, -d - 0.02, top + f), (half + f, d + 0.02, DECK), "Hull alloy")
        for side in (-1, 1):
            y0, y1 = sorted((side * d, side * (d + 0.08)))
            for x in (-half - f, half):
                p.span((x, y0, 0), (x + f, y1, top + f), "Crew grey")
            p.span((-half - f, y0, top), (half + f, y1, top + f), "Crew grey")
        p.span((-half - f, -d, 0), (-half, d, top + f), "Hull alloy")
        p.span((half, -d, 0), (half + f, d, top + f), "Hull alloy")
        p.span((-half, -d, top), (half, d, top + f), "Hull alloy")
        p.span((dx0, -0.05, 0.02), (half, 0.05, top - 0.02), "Repaint teal")
    return p


def _rebreather(p: Piece, at) -> None:
    """A half-mask rebreather hung by its strap: the rubber cup with its
    edge seal, an exhale valve and grille, two filter cans angled out on the
    cheeks, the crown strap up to the hook."""
    x, y, z = at
    rot = (90, 0, 0)
    p.lathe([(0.0, 0.0), (0.04, 0.0), (0.075, 0.035), (0.088, 0.075), (0.082, 0.1), (0.06, 0.118), (0.0, 0.122)], (x, y - 0.04, z), "Rubber",
            rot=rot, segments=6)
    p.torus(0.087, 0.008, (x, y - 0.115, z), "Hull dark", rot=rot, segments=6, sides=1.2)
    p.cyl(0.03, 0.035, (x, y - 0.17, z - 0.015), "Hull dark", rot=rot, segments=4)
    for k in range(3):
        p.box((0.04, 0.004, 0.006), (x, y - 0.19, z - 0.03 + k * 0.012), "Steel")
    for sx in (-1, 1):
        cx = x + sx * 0.085
        p.cyl(0.034, 0.07, (cx, y - 0.13, z - 0.02), "Crew orange", rot=(90, 0, sx * 32), segments=5)
        p.cyl(0.036, 0.012, (cx + sx * 0.02, y - 0.165, z - 0.02), "Hull dark", rot=(90, 0, sx * 32), segments=5)
    p.torus(0.09, 0.006, (x, y - 0.03, z + 0.07), "Hull dark", rot=(75, 0, 0), segments=5, sides=1.2, arc=180)
    p.cable((x, y - 0.03, z + 0.16), (x, y, z + 0.22), 0.006, "Hull dark", segments=3, steps=3)


def _filter(p: Piece, at) -> None:
    """A filter cartridge lying on its side, its pleated body, end caps and
    the Crew's orange band."""
    x, y, z = at
    rot = (90, 0, 0)
    p.lathe([(0.0, 0.0), (0.058, 0.0), (0.062, 0.01), (0.062, 0.03), (0.056, 0.035), (0.056, 0.205), (0.062, 0.21), (0.062, 0.23),
             (0.058, 0.24), (0.02, 0.24), (0.02, 0.26), (0.0, 0.26)], (x, y, z), "Paper aged", rot=rot, segments=5)
    with p.painted():
        p.cyl(0.062, 0.05, (x, y - 0.17, z), "Crew orange", rot=rot, segments=5)


def mask_station(style: Style) -> Piece:
    """A Crew mask station by the gate, where everyone kits up before
    stepping out into the Fringe: rebreathers on hooks, two racks of filter
    cartridges, a reader to pay at, the station's number on its orange
    header. Its back goes on a wall's face (y=0)."""
    p = Piece("mask_station", "prop", "0.9 × 0.3 × 1.3 m wall dispenser, 0.6 m off the floor")
    p.budget = 25000  # three masks and eight filters, rounded: a few by the gates
    w, d, z0, z1 = 0.45, 0.3, 0.6, 1.9
    # The cabinet: back panel, side cheeks, a shelf, the header.
    p.span((-w, -0.03, z0), (w, 0, z1 - 0.21), "Hull dark")
    for x in (-w, w - 0.04):
        p.span((x, -d, z0), (x + 0.04, -0.03, z1 - 0.2), "Crew grey")
    p.span((-w + 0.04, -d, z0), (w - 0.04, -0.03, z0 + 0.04), "Crew grey")
    p.span((-w + 0.04, -d, 1.15), (w - 0.04, -0.03, 1.18), "Crew grey")
    p.span((-w - 0.02, -d - 0.02, z1 - 0.2), (w + 0.02, -0.005, z1), "Crew orange")
    _bolts(p, [(sx * (w - 0.03), -d - 0.025, z1 - 0.1) for sx in (-1, 1)], 0.012, 0.01, "Steel", rot=(90, 0, 0))
    with p.painted():
        p.stencil("02", (-0.2, -d - 0.026, z1 - 0.1), 0.12, "Stencil white")
        p.span((0.02, -d - 0.028, z1 - 0.13), (0.38, -d - 0.024, z1 - 0.07), "Stencil white")
    p.collider((2 * w, d, z1 - z0), (0, -d / 2, (z0 + z1) / 2))
    # Three rebreathers hanging on hooks.
    for x in (-0.27, 0.0, 0.27):
        p.cyl(0.008, 0.09, (x, -0.075, 1.66), "Steel", rot=(90, 0, 0), segments=2.4)
        _rebreather(p, (x, -0.06, 1.44))
    # Filters on their sides, two rows of four on rails.
    for row in range(2):
        zr = 0.72 + row * 0.2
        p.span((-w + 0.04, -0.26, zr - 0.075), (w - 0.04, -0.24, zr - 0.06), "Steel")
        for i in range(4):
            _filter(p, (-0.3 + i * 0.2, -0.03, zr))
    # The reader on the side, its screen and slot.
    p.span((w, -0.14, 1.1), (w + 0.12, -0.02, 1.38), "Hull dark")
    p.span((w + 0.12, -0.12, 1.26), (w + 0.126, -0.04, 1.34), "Status blue")
    p.span((w + 0.12, -0.11, 1.15), (w + 0.124, -0.05, 1.16), "Rubber")
    return p


# The dome overhead. The renderer can't show glass as glass (nothing is see-
# through but what's cut out), so overhead the dome is an open geodesic
# frame, panes left only along its seams: fogged where they've aged,
# patched with sheet where they broke.
#
# How it fits together, every piece in the frame of the dome_wall it rises
# from (front -Y out to the Fringe, +Y in):
# - dome_wall's leaning glass tops out at y 1.18, z 5.07;
# - dome_roof_slope stands on the same origin (the same at and turns as the
#   wall below it), 4 m of it to a 4 m wall, and rises from there 8 m inward
#   to y 9.18, z 20.0 (ROOF);
# - dome_roof sections, 8 × 8 m, sit with their bases at z 20.0 from there
#   inward, their outer edge on the slope's top edge;
# - at a corner of the dome line, dome_corner joins the two runs of wall,
#   and dome_slope_hip closes the slope over it, on the same origin (where
#   the two walls' lines cross): it covers both runs out to 10 m along them,
#   so the first regular wall on each run is at 4 m and 8 m along it and the
#   first regular slope at 12 m.
TOP_Y = (WALL - FOOT) * math.tan(math.radians(LEAN))
TOP_Z = WALL + 0.07
ROOF = 20.0
REACH = TOP_Y + 8.0  # how far in the slope's top edge is
HIP = 10.0           # how far along each run the hip piece covers


def _sheet(p: Piece, corners, thickness: float, mat: str) -> None:
    """A flat sheet through four corners (a pane, a patch): a slab
    thickness through, either side of them."""
    a, b, c, d = (Vector(v) for v in corners)
    n = (b - a).cross(d - a).normalized() * (thickness / 2)
    top = [p.bm.verts.new(v + n) for v in (a, b, c, d)]
    bot = [p.bm.verts.new(v - n) for v in (a, b, c, d)]
    faces = [p.bm.faces.new(top), p.bm.faces.new(list(reversed(bot)))]
    for i in range(4):
        j = (i + 1) % 4
        faces.append(p.bm.faces.new((top[j], top[i], bot[i], bot[j])))
    p._faces(faces, mat, smooth=False)


def _strut(p: Piece, a, b, w: float = 0.22, h: float = 0.3, low: bool = False) -> None:
    """A strut of the frame from a to b: a box section with a capping strip
    (or, for the stand-in, just the box)."""
    _beam(p, a, b, w, h, "Hull alloy")
    if not low:
        a, b = Vector(a), Vector(b)
        d = (b - a).normalized()
        up = Vector((0, 0, 1)) if abs(d.z) < 0.95 else Vector((0, 1, 0))
        out = (up - d * up.dot(d)).normalized() * (h / 2 + 0.012)
        _beam(p, a + out + d * 0.12, b + out - d * 0.12, w * 0.45, 0.024, "Hull dark")


def _hub(p: Piece, at, normal, low: bool = False) -> None:
    """A cast hub where struts meet, bolted, facing along normal."""
    q = Vector((0, 0, 1)).rotation_difference(Vector(normal).normalized())
    rot = tuple(math.degrees(r) for r in q.to_euler())
    p.cyl(0.26, 0.34, at, "Hull dark", rot=rot, segments=2.4 if low else 4)
    if not low:
        p.cyl(0.12, 0.38, at, "Steel", rot=rot, segments=3)
        for k in range(6):
            t = k * math.tau / 6
            off = q @ Vector((math.cos(t) * 0.19, math.sin(t) * 0.19, 0.18))
            p.cyl(0.02, 0.03, Vector(at) + off, "Steel", rot=rot, segments=2.4)


def _slope_point(x: float, t: float):
    """A point on the slope, x along the wall and t from its foot (0) to its
    top (1)."""
    return (x, TOP_Y + (REACH - TOP_Y) * t, TOP_Z + (ROOF - TOP_Z) * t)


SLOPE_ROWS = (0.0, 0.34, 0.67, 1.0)


def dome_roof_slope(style: Style) -> Piece:
    """4 m of the dome rising from the lower wall's top to the roof: rafters
    and purlins of the geodesic frame, braced in triangles, climbing 15 m
    over 8 m inward. The bottom row keeps its panes, fogged and one patched
    with sheet, sealing the wall's top; above, the frame's open. Placed on
    the same origin as the dome_wall it rises from."""
    p = Piece("dome_roof_slope", "kit", "4 m of dome slope from the wall's top (y 1.18, z 5.07) to y 9.18, z 20; no collider",
              budget=800)
    r = rng(style, p.name)
    xs = (-2.0, 0.0, 2.0)

    def frame(low: bool):
        normal = Vector(_slope_point(0, 1)) - Vector(_slope_point(0, 0))
        normal = Vector((0, -normal.z, normal.y)).normalized()
        for i, x in enumerate(xs):
            # Halves at the ends, shared with the next slope along.
            w = 0.11 if i != 1 else 0.22
            cx = x + (0.055 if i == 0 else -0.055 if i == 2 else 0)
            _strut(p, _slope_point(cx, 0), _slope_point(cx, 1), w=w, low=low)
        for t in SLOPE_ROWS:
            _strut(p, _slope_point(-2.0, t), _slope_point(2.0, t), low=low)
        for k, (t0, t1) in enumerate(zip(SLOPE_ROWS, SLOPE_ROWS[1:])):
            for j, (x0, x1) in enumerate(zip(xs, xs[1:])):
                a, b = ((x0, t0), (x1, t1)) if (j + k) % 2 == 0 else ((x1, t0), (x0, t1))
                _strut(p, _slope_point(a[0], a[1]), _slope_point(b[0], b[1]), w=0.16, h=0.22, low=low)
        for x in (0.0,):
            for t in SLOPE_ROWS:
                _hub(p, _slope_point(x, t), normal, low=low)

    frame(low=False)
    # The bottom row's panes: glass fogged at its seals, one patched.
    t0, t1 = SLOPE_ROWS[0] + 0.03, SLOPE_ROWS[1] - 0.03
    patched = r.randrange(2)
    for j, (x0, x1) in enumerate(((-1.85, -0.15), (0.15, 1.85))):
        corners = [_slope_point(x0, t0), _slope_point(x1, t0), _slope_point(x1, t1), _slope_point(x0, t1)]
        _sheet(p, corners, 0.03, "Repaint oxide" if j == patched else "Glass fog")
    with p.painted():
        p.stencil("12", Vector(_slope_point(0.9, 0.15)) + Vector((0, -0.2, 0.0)), 0.22, "Stencil white")
    with p.lowpoly():
        frame(low=True)
        for x0, x1 in ((-1.85, -0.15), (0.15, 1.85)):
            _sheet(p, [_slope_point(x0, t0), _slope_point(x1, t0), _slope_point(x1, t1), _slope_point(x0, t1)],
                   0.04, "Glass fog")
    return p


def dome_roof(style: Style) -> Piece:
    """An 8 × 8 m section of the dome's roof: the geodesic frame laid flat
    overhead, struts in triangles between cast hubs, open to the sky but for
    a pane fogged on and a patch of sheet riveted in where one broke. Its
    origin is its base, placed at the roof's height (ROOF, 20 m); the edge
    struts are halves, so sections laid side by side share one."""
    p = Piece("dome_roof", "kit", "8 × 8 m overhead frame, base at its origin; tiles by 8 m; no collider", budget=800)
    r = rng(style, p.name)
    h = 0.3
    z = h / 2
    lines = (-4.0, 0.0, 4.0)

    def frame(low: bool):
        for i, c in enumerate(lines):
            w = 0.11 if i != 1 else 0.22
            off = 0.055 if i == 0 else -0.055 if i == 2 else 0.0
            _strut(p, (-4, c + off, z), (4, c + off, z), w=w, h=h, low=low)
            _strut(p, (c + off, -4, z), (c + off, 4, z), w=w, h=h, low=low)
        for j, (y0, y1) in enumerate(zip(lines, lines[1:])):
            for i, (x0, x1) in enumerate(zip(lines, lines[1:])):
                a, b = ((x0, y0), (x1, y1)) if (i + j) % 2 == 0 else ((x1, y0), (x0, y1))
                _strut(p, (a[0], a[1], z), (b[0], b[1], z), w=0.16, h=0.22, low=low)
        _hub(p, (0, 0, z), (0, 0, 1), low=low)
        for c in ((0, -4 + 0.3), (0, 4 - 0.3), (-4 + 0.3, 0), (4 - 0.3, 0)):
            _hub(p, (c[0], c[1], z), (0, 0, 1), low=low)

    frame(low=False)
    # A fogged pane over one triangle, a riveted patch over another.
    cells = [((0.15, 0.15), (3.85, 0.15), (3.85, 3.85)), ((-3.85, -0.15), (-0.15, -0.15), (-0.15, -3.85))]
    for k, tri in enumerate(cells):
        a, b, c = ((x, y, h + 0.02) for x, y in tri)
        _sheet(p, [a, b, c, c], 0.03, "Glass fog" if k == 0 else "Repaint oxide")
    with p.lowpoly():
        frame(low=True)
        for k, tri in enumerate(cells):
            a, b, c = ((x, y, h + 0.02) for x, y in tri)
            _sheet(p, [a, b, c, c], 0.04, "Glass fog")
    return p


def dome_slope_hip(style: Style) -> Piece:
    """The dome's slope where two runs of it meet at a corner of the dome
    line: each run's frame climbing to the hip between them, a heavy hip
    strut up the diagonal. On the same origin as the dome_corner below it
    (the walls' lines crossing); it covers both runs out to 10 m along them."""
    p = Piece("dome_slope_hip", "kit", "corner of the dome slope, 10 m along each run; no collider", budget=800)

    def run_point(x, t, mirror):
        px, py, pz = _slope_point(x, t)
        return (py, px, pz) if mirror else (px, py, pz)

    def hip_point(t):
        _, y, z = _slope_point(0, t)
        return (y, y, z)

    def frame(low: bool):
        _strut(p, hip_point(0), hip_point(1), w=0.3, h=0.36, low=low)
        for mirror in (False, True):
            for t in SLOPE_ROWS:
                x0 = TOP_Y + (REACH - TOP_Y) * t
                if x0 < HIP - 0.2:
                    _strut(p, run_point(x0, t, mirror), run_point(HIP, t, mirror), low=low)
            for x in (6.0, HIP - 0.055):
                # Rafters from where they meet the hip (or the foot) to the top.
                t_hip = max(0.0, min(1.0, (x - TOP_Y) / (REACH - TOP_Y)))
                w = 0.11 if x > HIP - 0.1 else 0.22
                _strut(p, run_point(x, 0.0 if x <= TOP_Y else 0.0, mirror) if x <= TOP_Y else run_point(x, 0.0, mirror),
                       run_point(x, t_hip, mirror), w=w, low=low)
            for (t0, t1) in zip(SLOPE_ROWS, SLOPE_ROWS[1:]):
                xa = TOP_Y + (REACH - TOP_Y) * t1
                if xa < HIP - 0.3:
                    _strut(p, run_point(HIP, t0, mirror), run_point(xa, t1, mirror), w=0.16, h=0.22, low=low)
        for t in SLOPE_ROWS:
            _hub(p, hip_point(t), (-1, -1, 1.2), low=low)

    frame(low=False)
    # The bottom row's panes, as the slope's, cut to the hip.
    t0, t1 = SLOPE_ROWS[0] + 0.03, SLOPE_ROWS[1] - 0.03

    def panes(low: bool):
        for mirror in (False, True):
            ha = TOP_Y + (REACH - TOP_Y) * t0 + 0.2
            hb = TOP_Y + (REACH - TOP_Y) * t1 + 0.2
            corners = [run_point(ha, t0, mirror), run_point(HIP - 0.15, t0, mirror), run_point(HIP - 0.15, t1, mirror),
                       run_point(hb, t1, mirror)]
            if mirror:
                corners.reverse()
            _sheet(p, corners, 0.04 if low else 0.03, "Glass fog" if low or not mirror else "Repaint oxide")

    panes(low=False)
    with p.lowpoly():
        frame(low=True)
        panes(low=True)
    return p


def dome_corner(style: Style) -> Piece:
    """Where two runs of the dome's lower wall meet at a right angle: an L of
    cast footing, and the two leaning walls of glass cut to meet along a hip
    mullion rising from the corner, so the two runs close without a gap or an
    overlap. Its origin is where the two walls' lines cross; its arms run 2 m
    along +X and +Y (inside the dome is +X, +Y), to meet the first walls of
    each run (at 4 m along, the X run as laid, the Y run turned three
    quarters)."""
    p = Piece("dome_corner", "kit", "dome wall corner: arms 2 m along +X and +Y, inside +X +Y", budget=500)
    p.budget = 500
    r = rng(style, p.name)
    arm = 2.0
    length = (WALL - FOOT) / math.cos(math.radians(LEAN))
    sin, cos = math.sin(math.radians(LEAN)), math.cos(math.radians(LEAN))
    footing = [(-0.4, 0.0), (0.4, 0.0), (0.4, FOOT - 0.12), (0.32, FOOT - 0.02), (-0.32, FOOT - 0.02), (-0.4, FOOT - 0.12)]

    def x_arm(s, x, off=0.0):
        """A point on the X run's glass: s up it, x along it, off inward."""
        return Vector((x, s * sin + off * cos, FOOT + s * cos - off * sin))

    def y_arm(s, y, off=0.0):
        v = x_arm(s, y, off)
        return Vector((v.y, v.x, v.z))

    def walls(low: bool):
        # The footing: an L, the X arm reaching back over the corner.
        p.prism(footing, arm + 0.4, (arm / 2 - 0.2, 0, 0), "Red concrete", rot=(0, 0, 90))
        p.prism(footing, arm - 0.4, (0, arm / 2 + 0.2, 0), "Red concrete")
        # The glass of each arm, cut along the hip (x = s sin LEAN).
        for side, at in ((0, x_arm), (1, y_arm)):
            tops = [(at(0, 0.0)), at(0, arm), at(length, arm), at(length, length * sin)]
            _sheet(p, tops, 0.03 if not low else 0.24, "Glass")
            if not low:
                # Transoms across each arm, from the hip to its end.
                for s in (0.0, length * 0.36, length * 0.7, length):
                    _beam(p, at(s, s * sin + 0.05), at(s, arm - 0.012), 0.2, 0.14, "Hull dark")
                # The end mullion (half, shared with the next wall), a little
                # past the glass at both ends so their ends don't share a plane.
                _beam(p, at(-0.03, arm - 0.04), at(length + 0.03, arm - 0.04), 0.18, 0.07, "Hull dark")
        if not low:
            # The hip mullion up the corner, capped and bolted.
            _beam(p, x_arm(-0.04, -0.04 * sin), x_arm(length + 0.04, (length + 0.04) * sin), 0.24, 0.24, "Hull dark")
            _beam(p, x_arm(0.06, 0.0, -0.13), x_arm(length - 0.06, length * sin, -0.13), 0.1, 0.03, "Hull alloy")

    walls(low=False)
    # Fog at the panes' seals: bands along each arm's transoms and hip.
    for at in (x_arm, y_arm):
        for s0, s1 in ((0.07, 0.25), (length * 0.36 + 0.07, length * 0.36 + 0.22), (length * 0.7 + 0.07, length * 0.7 + 0.2)):
            _sheet(p, [at(s0, s0 * sin + 0.12, -0.02), at(s0, arm - 0.1, -0.02), at(s1, arm - 0.1, -0.02),
                       at(s1, s1 * sin + 0.12, -0.02)], 0.01, "Glass fog")
    p.span((-0.4 + 0.005, -0.3, FOOT - 0.02), (arm - 0.005, 0.3, FOOT), "Steel")
    p.span((-0.3, 0.3, FOOT - 0.02), (0.3, arm - 0.005, FOOT), "Steel")
    with p.painted():
        p.stencil("1", (1.0, -0.405, 0.32), 0.16, "Stencil white")
    # Dust banked into the outside corner.
    p.prism([(-0.6, 0.0), (arm, 0.0), (arm, 0.08), (0.0, 0.26), (-0.6, 0.12)], 0.3, (0, -0.56, 0), "Dust red")
    p.prism([(-0.6, 0.0), (arm, 0.0), (arm, 0.08), (0.0, 0.26), (-0.6, 0.12)], 0.3, (-0.56, 0, 0), "Dust red",
            rot=(0, 0, 90))
    p.collider((arm + 0.4, 0.8, FOOT), (arm / 2 - 0.2, 0, FOOT / 2))
    p.collider((0.8, arm - 0.4, FOOT), (0, arm / 2 + 0.2, FOOT / 2))
    mid = x_arm(length / 2, arm / 2 + length / 2 * sin / 2)
    # From 0.3 m along (clear of the hip) to the arm's end.
    p.collider((arm - 0.3, 0.2, length), ((arm + 0.3) / 2, mid.y, mid.z), rot=(-LEAN, 0, 0))
    p.collider((0.2, arm - 0.3, length), (mid.y, (arm + 0.3) / 2, mid.z), rot=(0, LEAN, 0))
    with p.lowpoly():
        walls(low=True)
        p.prism([(-0.6, 0.0), (arm, 0.0), (arm, 0.08), (0.0, 0.26), (-0.6, 0.12)], 0.3, (0, -0.56, 0), "Dust red")
        p.prism([(-0.6, 0.0), (arm, 0.0), (arm, 0.08), (0.0, 0.26), (-0.6, 0.12)], 0.3, (-0.56, 0, 0), "Dust red",
                rot=(0, 0, 90))
    del r
    return p


PIECES = [dome_wall, dome_frame, dome_strut_anchor, south_gate, boom_barrier, checkpoint_booth,
          scanner_arch, airlock_door, mask_station, dome_roof_slope, dome_roof, dome_slope_hip, dome_corner]
