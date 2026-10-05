"""The Fringe: everything outside the domes, where the masks go on and the
law stops (docs/settlement.md). Red dust and rock, wind farms, patched
greenhouses and grain beds, the Fringers' camps and salvage digs, the hardy
plants of a half-terraformed world, and the terraformers Corvane left
half-built, wrecked on the horizon. Everything is sun-bleached, rusted,
patched and red with dust.

The rocks and plants are Poly Haven's photoscans, recoloured for the Red;
the rest is modelled. Each piece stands on its origin, its front looking
along -Y (see GUIDE.md)."""
import math

from mathutils import Vector

from kit import PALETTE, REPAINTS, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "The Fringe"

# The Fringe's colours: paint the sun has had (a hard surface, unlike the
# finish's cloth "Bleached"), greenhouse film (cloth, roughened in the
# finish), scorch and soot, leaves and rope.
PALETTE.setdefault("Sunbleached paint", (0.66, 0.60, 0.48))
PALETTE.setdefault("Poly film", (0.52, 0.60, 0.52))
PALETTE.setdefault("Poly film old", (0.60, 0.53, 0.30))
PALETTE.setdefault("Char", (0.06, 0.04, 0.03))
PALETTE.setdefault("Corvane faded", (0.55, 0.52, 0.46))
PALETTE.setdefault("Ember", (1.0, 0.30, 0.05))
PALETTE.setdefault("Leaf green", (0.14, 0.26, 0.06))
PALETTE.setdefault("Rope", (0.48, 0.38, 0.22))

ROCK = "Rock red"


# Helpers -----------------------------------------------------------------------

def _loft(p: Piece, rings, mat: str, smooth=True, caps=True) -> None:
    """A skin through rings of points (each the same count, in order round
    it): blades, socks, tapering hulls."""
    verts = [[p.bm.verts.new(Vector(v)) for v in ring] for ring in rings]
    n = len(rings[0])
    faces = []
    for a, b in zip(verts, verts[1:]):
        for i in range(n):
            j = (i + 1) % n
            faces.append(p.bm.faces.new((a[i], a[j], b[j], b[i])))
    if caps:
        faces.append(p.bm.faces.new(list(reversed(verts[0]))))
        faces.append(p.bm.faces.new(verts[-1]))
    p._faces(faces, mat, smooth)


def _drift(p: Piece, at, rx: float, ry: float, h: float, mat: str, rand, rings=6, segments=16,
           lumps: int = 3, foot: float = -0.05) -> None:
    """A drift of dust: an ellipse rx by ry heaped to h, lumpy, its edge sunk
    to foot. Built ring by ring, so its triangles are rings × segments × 2."""
    from mathutils import noise
    seed = Vector((rand.uniform(0, 50), rand.uniform(0, 50), 0))
    bumps = [(rand.uniform(-0.5, 0.5), rand.uniform(-0.5, 0.5), rand.uniform(0.5, 1.0)) for _ in range(lumps)]
    at = Vector(at)

    def z(u, v):
        r = math.hypot(u, v)
        if r >= 1:
            return foot
        body = (1 - r * r) ** 1.6
        for bx, by, s in bumps:
            body += 0.2 * s * math.exp(-((u - bx) ** 2 + (v - by) ** 2) * 6) * (1 - r)
        body *= 1 + 0.18 * noise.noise(Vector((u * 2.3, v * 2.3, 0)) + seed)
        return max(foot, foot + (h - foot) * body)

    centre = p.bm.verts.new(at + Vector((0, 0, z(0, 0))))
    ring_verts = []
    for k in range(1, rings + 1):
        rr = k / rings
        ring = []
        for s in range(segments):
            a = math.tau * s / segments
            u, v = math.cos(a) * rr, math.sin(a) * rr
            ring.append(p.bm.verts.new(at + Vector((u * rx, v * ry, z(u * 0.999, v * 0.999) if k < rings else foot))))
        ring_verts.append(ring)
    faces = []
    for s in range(segments):
        t = (s + 1) % segments
        faces.append(p.bm.faces.new((centre, ring_verts[0][s], ring_verts[0][t])))
    for a, b in zip(ring_verts, ring_verts[1:]):
        for s in range(segments):
            t = (s + 1) % segments
            faces.append(p.bm.faces.new((a[s], b[s], b[t], a[t])))
    p._faces(faces, mat, smooth=True)


def _pebbles(p: Piece, rand, around, spread: float, count: int, size=(0.05, 0.14)) -> None:
    """Loose stones scattered round around (x, y), up to spread out: faceted
    lumps, half sunk."""
    import bmesh
    from kit import _euler
    from mathutils import Matrix
    for _ in range(count):
        a, d = rand.uniform(0, math.tau), rand.uniform(0.3, 1.0) * spread
        s = rand.uniform(*size)
        res = bmesh.ops.create_icosphere(p.bm, subdivisions=1, radius=1.0)
        m = Matrix.Translation((around[0] + math.cos(a) * d, around[1] + math.sin(a) * d, s * 0.25)) @ \
            _euler((0, 0, rand.uniform(0, 360))) @ Matrix.Diagonal((s * 1.3, s, s * 0.7, 1))
        for v in res["verts"]:
            v.co = m @ (v.co * rand.uniform(0.8, 1.2))
        p._take(res["verts"], ROCK)


def _bolts(p: Piece, points, radius: float, depth: float, mat: str, rot=(0, 0, 0)) -> None:
    """Bolt heads (or rivets) at points, standing depth proud: painted, so
    they're baked into the texture rather than drawn (dozens of tiny
    bevelled cylinders would be most of a piece's triangles)."""
    with p.painted():
        for at in points:
            p.cyl(radius, depth, at, mat, rot=rot, segments=6)


def _ring_points(r: float, n: int, z: float, start=0.0):
    return [(math.cos(start + math.tau * i / n) * r, math.sin(start + math.tau * i / n) * r, z) for i in range(n)]


# Rocks and dust -----------------------------------------------------------------

def rock_small(style: Style) -> Piece:
    """A knee-high rock, the kind the Fringe is strewn with: a split block of
    red sandstone, low enough to vault (0.7 m high, 0.7 m deep)."""
    p = Sourced("rock_small", "prop", "1.3 × 0.73 m rock, 0.7 m high: vaultable",
                asset="namaqualand_boulder_06", res="1k", height=0.7, turns=1, recolour=ROCK, budget=2500)
    p.collider((1.1, 0.6, 0.7), (0, 0, 0.35))
    return p


def rock_large(style: Style) -> Piece:
    """A flat-topped boulder 1.55 m high, to mantle onto, with a smaller rock
    beside it at 0.7 m to vault up from."""
    p = Piece("rock_large", "prop", "about 4.4 × 2.6 m: boulder with a flat top at 1.55 m (mantle), 0.7 m step rock",
              budget=6000)
    p.source("namaqualand_boulder_03", at=(-0.4, 0, 0), rot=(0, 0, 90), height=1.55, recolour=ROCK)
    p.source("namaqualand_boulder_05", at=(2.05, -0.55, 0), rot=(0, 0, 8), height=0.7, recolour=ROCK)
    # Stones split off it, round its foot (and something of its own for the
    # piece to be built on: a piece of sources alone loses a material).
    _pebbles(p, rng(style, p.name), (0.3, 0), 2.6, 9)
    p.collider((2.7, 2.1, 1.55), (-0.4, 0, 0.775))
    p.collider((1.4, 0.75, 0.7), (2.05, -0.55, 0.35))
    return p


def rock_spire(style: Style) -> Piece:
    """A wind-carved hoodoo of red rock, 6 m tall: blocks stacked as the
    storms left them, a landmark on the long sightlines out of the domes."""
    p = Piece("rock_spire", "prop", "about 3.5 × 3.4 m, 6.2 m tall", budget=10000)
    p.source("namaqualand_boulder_04", at=(0, 0, 0), height=2.6, recolour=ROCK)
    p.source("namaqualand_boulder_03", at=(0.15, 0.05, 2.25), rot=(90, 0, 30), height=2.6, recolour=ROCK)
    p.source("namaqualand_boulder_06", at=(0.25, 0.1, 4.75), rot=(90, 0, 70), height=1.45, recolour=ROCK)
    p.source("namaqualand_boulders_01", at=(1.9, -1.4, 0), rot=(0, 0, 40), height=0.32, recolour=ROCK)
    _pebbles(p, rng(style, p.name), (0, 0), 2.4, 12)
    p.collider((2.8, 2.8, 2.4), (0, 0, 1.2))
    p.collider((1.6, 1.0, 2.4), (0.15, 0.05, 3.6))
    return p


def dust_mound(style: Style) -> Piece:
    """A drift of red dust banked up by the storms, a few stones poking out.
    Soft: it has no collider, so it's walked through. What the game draws is
    a lean heap; its ripples and stones are baked onto it."""
    p = Piece("dust_mound", "prop", "about 4.6 × 3 m, 0.45 m high; no collider", budget=600)
    r = rng(style, p.name)
    _drift(p, (0, 0, 0), 2.3, 1.5, 0.45, "Dust red", r, rings=14, segments=48, lumps=4)
    for _ in range(6):
        a, d = r.uniform(0, math.tau), r.uniform(0.3, 0.75)
        x, y = math.cos(a) * d * 2.3, math.sin(a) * d * 1.5
        s = r.uniform(0.08, 0.16)
        p.sphere(s, (x, y, 0.45 * (1 - d * d) ** 1.6 - 0.03), "Rock dark", scale=(1.2, 1.0, 0.7),
                 rot=(0, 0, r.uniform(0, 180)), segments=7, rings=5)
    with p.lowpoly():
        _drift(p, (0, 0, 0), 2.3, 1.5, 0.45, "Dust red", rng(style, p.name), rings=7, segments=20, lumps=4)
    return p


# The wind farm -----------------------------------------------------------------

def _blade(p: Piece, hub: Vector, angle: float, length: float, mat: str) -> None:
    """One rotor blade from the hub out at angle (degrees round the rotor
    face, which looks along -Y): a twisted aerofoil, broad at its root and
    tapering to a tip."""
    a = math.radians(angle)
    out = Vector((math.cos(a), 0, math.sin(a)))
    across = Vector((-math.sin(a), 0, math.cos(a)))
    face = Vector((0, -1, 0))
    rings = []
    sections = 9
    for i in range(sections + 1):
        t = i / sections
        r = 0.35 + t * length
        chord = 0.62 * (1 - t) ** 0.8 + 0.12 if t > 0.12 else 0.32 + t * 2.5
        thick = chord * (0.28 if t < 0.12 else 0.16)
        twist = math.radians(22 * (1 - t))
        ring = []
        for k in range(10):
            th = math.tau * k / 10
            x = math.cos(th) * 0.5 - 0.15  # rounder at the leading edge
            x = x * (1.0 if x > 0 else 1.4)
            y = math.sin(th) * 0.5 * (1 - 0.6 * max(0.0, -math.cos(th)))
            cx, cy = x * chord, y * thick
            v = across * (cx * math.cos(twist)) + face * (cx * math.sin(twist) + cy)
            ring.append(hub + out * r + v)
        rings.append(ring)
    _loft(p, rings, mat)


def wind_turbine(style: Style) -> Piece:
    """One of the Fringe's wind turbines: an 18 m tapering mast on a concrete
    pad, a caged ladder up its back to the nacelle, the nacelle patched over
    in whatever was going, and three blades, one a mismatched replacement."""
    p = Piece("wind_turbine", "prop", "18.5 m wind turbine; mast collider", budget=60000)
    r = rng(style, p.name)
    mast = 12.4
    # The pad: an octagon of concrete, its anchor bolts, dust banked up.
    p.lathe([(0, 0), (1.25, 0), (1.25, 0.25), (1.05, 0.42), (0, 0.42)], (0, 0, 0), "Concrete", segments=8, smooth=False)
    p.lathe([(0.52, 0.42), (0.55, 0.5), (0.55, 0.62), (0.47, 0.66), (0, 0.66)], (0, 0, 0), "Hull dark", segments=12)
    _bolts(p, _ring_points(0.5, 12, 0.68), 0.03, 0.05, "Steel")
    _drift(p, (0.3, -0.4, 0), 1.9, 1.6, 0.32, "Dust red", r, rings=5, segments=20, lumps=2, foot=0.05)
    # The mast: three tapering sections flanged together.
    sections = [(0.62, 4.5, 0.44, 0.40), (4.5, 8.6, 0.40, 0.34), (8.6, mast, 0.34, 0.28)]
    for z0, z1, r0, r1 in sections:
        p.lathe([(r0, z0), (r1, z1)], (0, 0, 0), "Sunbleached paint", segments=16)
        p.torus(r1 + 0.03, 0.035, (0, 0, z1 - 0.02), "Sunbleached paint", segments=16, sides=6)
        _bolts(p, _ring_points(r1 + 0.05, 16, z1 - 0.02), 0.012, 0.05, "Steel")
    with p.painted():
        # Dust up the mast's foot, and its number.
        p.lathe([(0.446, 0.64), (0.432, 2.0)], (0, 0, 0), "Dust red", segments=16)
        p.stencil("03", (0, -0.452, 3.1), 0.32, "Hull dark")
    # The door at its foot, and the cable conduit up its side.
    p.span((-0.26, -0.46, 0.8), (0.26, -0.4, 2.7), "Sunbleached paint")
    p.span((-0.22, -0.48, 0.84), (0.22, -0.455, 2.66), r.choice(REPAINTS))
    for z in (1.0, 2.4):
        p.cyl(0.025, 0.12, (0.25, -0.47, z), "Steel", segments=8)
    p.box((0.05, 0.03, 0.18), (-0.15, -0.49, 1.7), "Hull dark")
    p.cable((0.3, -0.33, 0.66), (0.24, -0.27, mast), 0.03, "Rubber", segments=6, steps=4)
    # The ladder up the back (+Y), with its safety cage.
    ladder_y = 0.44
    for x in (-0.2, 0.2):
        p.tube([(x, ladder_y + 0.02, 0.7), (x, 0.3 + 0.06, mast + 0.1)], 0.02, "Rust", segments=6)
    z = 1.0
    while z < mast:
        y = ladder_y - (ladder_y - 0.34) * (z / mast) + 0.02
        p.box((0.4, 0.03, 0.03), (0, y, z), "Rust")
        z += 0.3
    z = 2.6
    while z < mast - 0.4:
        y = ladder_y - (ladder_y - 0.34) * (z / mast) + 0.36
        p.torus(0.36, 0.015, (0, y, z), "Rust", rot=(0, 0, 0), segments=12, sides=4, arc=180)
        z += 1.2
    for x in (-0.36, 0.0, 0.36):
        p.tube([(x, ladder_y + 0.36 + (0.0 if x else 0.36), 2.6), (x, 0.34 + 0.36 + (0.0 if x else 0.36), mast - 0.4)],
               0.012, "Rust", segments=4)
    # The yaw ring and the nacelle: a long rounded housing, patched.
    top = mast + 0.05
    p.cyl(0.36, 0.18, (0, 0, top + 0.09), "Hull dark", segments=16)
    rings = []
    for y, w, h in ((-0.95, 0.42, 0.42), (-0.6, 0.55, 0.55), (0.6, 0.6, 0.6), (1.6, 0.56, 0.55), (2.1, 0.42, 0.42)):
        ring = []
        for k in range(16):
            th = math.tau * k / 16
            cx, cz = math.cos(th), math.sin(th)
            # A rounded rectangle: squarer than an ellipse.
            sx = math.copysign(abs(cx) ** 0.6, cx) * w
            sz = math.copysign(abs(cz) ** 0.6, cz) * h
            ring.append((sx, y, top + 0.75 + sz))
        rings.append(ring)
    _loft(p, rings, "Sunbleached paint")
    # Patches welded over storm damage, a centimetre proud.
    for k, (y, side) in enumerate(((-0.2, -1), (0.6, -1), (0.2, 1))):
        p.box((0.012, r.uniform(0.35, 0.6), r.uniform(0.25, 0.45)), (side * (0.625 + 0.015 * k), y, top + 0.75 + r.uniform(-0.15, 0.2)),
              r.choice(REPAINTS + ["Rust"]))
    # Louvres down one side, a radiator at the back, a hatch on top.
    for i in range(6):
        p.box((0.04, 0.5, 0.03), (0.6, 1.1, top + 0.5 + i * 0.08), "Hull dark", rot=(25, 0, 0))
    p.span((-0.4, 2.12, top + 0.5), (0.4, 2.22, top + 1.05), "Hull dark")
    for i in range(9):
        x = -0.36 + i * 0.09
        p.span((x - 0.01, 2.22, top + 0.52), (x + 0.01, 2.32, top + 1.03), "Steel")
    p.span((-0.3, 0.2, top + 1.32), (0.3, 0.9, top + 1.38), r.choice(REPAINTS))
    # The weather mast: an anemometer and a vane, and its warning lamp.
    p.cyl(0.02, 0.6, (0, 1.7, top + 1.6), "Steel", segments=6)
    for i in range(3):
        a = math.tau * i / 3
        p.tube([(0, 1.7, top + 1.9), (math.cos(a) * 0.18, 1.7 + math.sin(a) * 0.18, top + 1.9)], 0.008, "Steel", segments=4)
        p.sphere(0.04, (math.cos(a) * 0.18, 1.7 + math.sin(a) * 0.18, top + 1.9), "Hull dark", segments=8, rings=4)
    p.prism([(0, 0), (0.25, 0.08), (0.25, -0.08)], 0.01, (0.0, 1.95, top + 1.7), "Hull dark", rot=(0, 0, 90))
    p.sphere(0.06, (0, 1.4, top + 1.45), "Medical red", segments=8, rings=6)
    # The hub: a spinner, and three blades, one a replacement in other paint.
    hub = Vector((0, -1.5, top + 0.75))
    p.lathe([(0.0, 0.0), (0.38, 0.05), (0.42, 0.3), (0.3, 0.55), (0.12, 0.7), (0, 0.74)], hub + Vector((0, 0.53, 0)),
            "Sunbleached paint", rot=(90, 0, 0), segments=16)
    p.cyl(0.45, 0.1, hub + Vector((0, 0.56, 0)), "Hull dark", rot=(90, 0, 0), segments=16)
    for i in range(3):
        _blade(p, hub, 120 * i + 20, 5.6, "Repaint oxide" if i == 2 else "Sunbleached paint")
    p.collider((0.8, 0.8, mast), (0, 0, mast / 2))
    return p


# Farms --------------------------------------------------------------------------

def _arch_sheet(p: Piece, radius: float, leg: float, y0: float, y1: float, mat: str, rand, sag=0.06, cells=(14, 6),
                thickness=0.01, lo=0.0, hi=math.pi) -> None:
    """Film stretched over the hoops from y0 to y1: an arch of radius on legs
    leg high, the film down the legs to the ground, sagging between hoops."""
    from mathutils import noise
    nu, nv = cells
    outer, inner = [], []
    seed = rand.uniform(0, 100)
    # The profile round the arch, down the legs at both ends.
    # Down the legs to the ground only where the film reaches them: a side
    # rolled up stops at its roll.
    prof = ([(math.cos(lo) * radius, 0.0)] if lo <= 0 else []) + \
           [(math.cos(lo + (hi - lo) * i / nu) * radius, leg + math.sin(lo + (hi - lo) * i / nu) * radius) for i in range(nu + 1)] + \
           ([(math.cos(hi) * radius, 0.0)] if hi >= math.pi else [])
    for j in range(nv + 1):
        v = j / nv
        y = y0 + (y1 - y0) * v
        dip = sag * math.sin(math.pi * v)
        row_o, row_i = [], []
        for i, (x, z) in enumerate(prof):
            n = Vector((x, 0, z - leg if z > leg else 0)).normalized() if (x or z > leg) else Vector((1, 0, 0))
            wob = 0.012 * noise.noise(Vector((x * 2.0, y * 2.0, seed)))
            pt = Vector((x, y, z)) - n * (dip * (1 if 0 < i < len(prof) - 1 else 0.3)) + n * wob
            row_o.append(p.bm.verts.new(pt + n * thickness / 2))
            row_i.append(p.bm.verts.new(pt - n * thickness / 2))
        outer.append(row_o)
        inner.append(row_i)
    faces = []
    m = len(prof)
    for j in range(nv):
        for i in range(m - 1):
            faces.append(p.bm.faces.new((outer[j][i], outer[j][i + 1], outer[j + 1][i + 1], outer[j + 1][i])))
            faces.append(p.bm.faces.new((inner[j][i], inner[j + 1][i], inner[j + 1][i + 1], inner[j][i + 1])))
    for j in range(nv):
        for i in (0, m - 1):
            faces.append(p.bm.faces.new((outer[j][i], outer[j + 1][i], inner[j + 1][i], inner[j][i])))
    for i in range(m - 1):
        for j in (0, nv):
            faces.append(p.bm.faces.new((outer[j][i], inner[j][i], inner[j][i + 1], outer[j][i + 1])))
    p._faces(faces, mat, smooth=True)


def _plant(p: Piece, at, rand, kind: str) -> None:
    """One crop plant standing at at: grain (three stalks, each with its ear
    and a leaf) or greens (a head in a rosette of leaves). Flat blades and
    crossed ears, a few dozen triangles a plant, left plain by the finish:
    there are hundreds of them."""
    foot = Vector(at)
    with p.plain():
        if kind == "grain":
            for k in range(3):
                yaw = rand.uniform(0, 360)
                tilt = rand.uniform(-12, 12)
                tall = rand.uniform(0.38, 0.55)
                base = foot + Vector((rand.uniform(-0.03, 0.03), rand.uniform(-0.03, 0.03), 0))
                rot = (0, tilt, yaw)
                # The stalk, tapering, and its ear: two crossed diamonds.
                p.prism([(-0.004, 0), (0.004, 0), (0.0015, tall), (-0.0015, tall)], 0.002, base, "Leaf green", rot=rot)
                top = base + (_euler_vec(rot) @ Vector((0, 0, tall)))
                for turn in (0, 90):
                    p.prism([(0, -0.01), (0.013, 0.03), (0, 0.085), (-0.013, 0.03)], 0.003, top, "Wheat",
                            rot=(0, tilt, yaw + turn))
                p.prism([(0, 0), (0.025, 0.1), (0.0, 0.2), (-0.01, 0.1)], 0.002, base + Vector((0, 0, tall * 0.25)),
                        "Leaf green", rot=(rand.uniform(-40, 40), rand.uniform(-50, 50), yaw))
        else:
            for k in range(7):
                a = math.tau * k / 7 + rand.uniform(-0.2, 0.2)
                p.prism([(-0.05, 0), (0.05, 0), (0.07, 0.14), (0.0, 0.22), (-0.07, 0.14)], 0.004, foot, "Leaf green",
                        rot=(rand.uniform(30, 60), 0, math.degrees(a) + 90))
            # The head: a few leaves cupped round, as crossed rounded blades.
            for k in range(3):
                p.prism([(-0.06, 0), (0.06, 0), (0.07, 0.07), (0.035, 0.13), (-0.035, 0.13), (-0.07, 0.07)], 0.004,
                        foot, "Crop green", rot=(rand.uniform(-8, 8), 0, 60 * k))


def _euler_vec(rot):
    from mathutils import Euler
    return Euler([math.radians(a) for a in rot]).to_matrix()


def _plants(p: Piece, a, b, z, rand, kind: str) -> None:
    """A row of crops on soil at height z, from (x, y) a to (x, y) b."""
    a, b = Vector((*a, z)), Vector((*b, z))
    n = max(2, round((b - a).length / (0.22 if kind == "grain" else 0.42)))
    for i in range(n):
        _plant(p, a.lerp(b, (i + 0.5) / n), rand, kind)


def greenhouse(style: Style) -> Piece:
    """A Fringe polytunnel, 6 × 4 m: five hoops of pipe on legs, film
    stretched over them and patched with tarp wherever a storm tore it, two
    beds of crops inside, a framed door at the front with the film rolled up
    over it. Its tarp patches and tape are paint, and its crops lean
    (see _plant)."""
    p = Piece("greenhouse", "prop", "4.1 × 6 m polytunnel, 2.65 m high, open at the front", budget=28000)
    r = rng(style, p.name)
    w, leg, length = 2.0, 0.6, 6.0
    hoops = [-length / 2 + i * length / 4 for i in range(5)]
    # Hoops, purlins and the bracing.
    for y in hoops:
        # Thin pipe: few sides (times DETAIL), and plain, unbevelled.
        with p.plain():
            p.torus(w, 0.035, (0, y, leg), "Steel", rot=(90, 0, 0), segments=12, sides=3, arc=180)
            for x in (-w, w):
                p.cyl(0.035, leg + 0.3, (x, y, (leg - 0.3) / 2), "Steel", segments=3)
    with p.plain():
        for a in (math.pi / 2, math.pi / 4, 3 * math.pi / 4):
            p.tube([(math.cos(a) * (w - 0.05), -length / 2, leg + math.sin(a) * (w - 0.05)),
                    (math.cos(a) * (w - 0.05), length / 2, leg + math.sin(a) * (w - 0.05))], 0.025, "Steel", segments=3)
    # The film, bay by bay, yellowing unevenly; one bay's side rolled up.
    for bay in range(4):
        mat = r.choice(["Poly film", "Poly film", "Poly film old"])
        hi = math.pi if bay != 2 else math.pi * 0.72
        _arch_sheet(p, w + 0.04, leg, hoops[bay] + 0.01, hoops[bay + 1] - 0.01, mat, r, hi=hi, lo=0.0)
    p.cyl(0.07, length / 4 - 0.1, (math.cos(math.pi * 0.72) * (w + 0.06), (hoops[2] + hoops[3]) / 2,
                                   leg + math.sin(math.pi * 0.72) * (w + 0.06)), "Poly film old", rot=(90, 0, 0), segments=10)
    with p.painted():
        # Tarp and canvas patches, and tape along the seams.
        for _ in range(7):
            a = r.uniform(0.25, math.pi - 0.25)
            y = r.uniform(-2.6, 2.6)
            rr = w + 0.06
            p.box((0.5, r.uniform(0.4, 0.8), 0.008), (math.cos(a) * rr, y, leg + math.sin(a) * rr),
                  r.choice(["Canvas", "Fabric teal", "Fabric red", "Fabric ochre"]), rot=(0, -(math.degrees(a) + 90), 0))
        for y in hoops:
            p.torus(w + 0.055, 0.012, (0, y, leg), "Hull dark", rot=(90, 0, 0), segments=20, sides=4, arc=180)
    # The back: film on a timber frame.
    back = [(-w, 0), (w, 0), (w, leg)] + [(math.cos(a) * w, leg + math.sin(a) * w) for a in (math.pi * i / 10 for i in range(1, 10))] + [(-w, leg)]
    p.prism(back, 0.02, (0, length / 2 + 0.05, 0), "Poly film old")
    for x in (-0.9, 0.9):
        p.span((x - 0.04, length / 2 - 0.04, 0), (x + 0.04, length / 2, leg + math.sqrt(max(0, w * w - x * x)) - 0.05), "Wood")
    # The front: a door frame, the film rolled up over it, a bucket by it.
    for x in (-0.6, 0.6):
        p.span((x - 0.045, -length / 2 - 0.045, 0), (x + 0.045, -length / 2 + 0.045, 2.15), "Wood")
    p.span((-0.65, -length / 2 - 0.045, 2.1), (0.65, -length / 2 + 0.045, 2.19), "Wood")
    p.cyl(0.13, 2.6, (0, -length / 2 - 0.08, leg + w - 0.25), "Poly film old", rot=(0, 90, 0), segments=12)
    for x in (-0.9, 0.9):
        p.cable((x, -length / 2 - 0.08, leg + w - 0.12), (x, -length / 2 - 0.08, leg + w - 0.4), 0.008, "Rope", steps=3)
    p.lathe([(0.0, 0), (0.13, 0), (0.17, 0.3), (0.16, 0.3), (0.12, 0.02), (0, 0.02)], (1.2, -length / 2 - 0.35, 0),
            "Hull alloy", segments=12)
    # Dust banked against the sides.
    for x in (-w - 0.15, w + 0.15):
        _drift(p, (x, 0.2, 0), 0.45, 2.9, 0.32, "Dust red", r, rings=4, segments=14, lumps=2)
    # Two beds of crops inside, a path between.
    for x, kind in ((-1.0, "greens"), (1.0, "grain")):
        p.span((x - 0.45, -2.6, 0), (x + 0.45, 2.6, 0.32), "Wood")
        p.span((x - 0.4, -2.55, 0.32), (x + 0.4, 2.55, 0.34), "Soil")
        for row in (-0.2, 0.2):
            _plants(p, (x + row, -2.4), (x + row, 2.4), 0.34, r, kind)
    p.collider((0.2, length, 1.6), (-w + 0.05, 0, 0.8))
    p.collider((0.2, length, 1.6), (w - 0.05, 0, 0.8))
    p.collider((2 * w, 0.1, 2.0), (0, length / 2, 1.0))
    return p


def _shell(p: Piece, radius, leg, length, hoops) -> None:
    """The polytunnel's film as a lean shell (for the stand-in): an arch of
    twelve faces down to the ground, a slight dip between hoops."""
    nu = 12
    prof = [(radius, 0.0)] + [(math.cos(math.pi * i / nu) * radius, leg + math.sin(math.pi * i / nu) * radius) for i in range(nu + 1)] + [(-radius, 0.0)]
    rows = []
    ys = []
    for a, b in zip(hoops, hoops[1:]):
        ys += [a, (a + b) / 2]
    ys.append(hoops[-1])
    for k, y in enumerate(ys):
        dip = 0.05 if k % 2 else 0.0
        row_o, row_i = [], []
        for x, z in prof:
            n = Vector((x, 0, max(0.0, z - leg))).normalized()
            pt = Vector((x, y, z)) - n * dip
            row_o.append(p.bm.verts.new(pt + n * 0.005))
            row_i.append(p.bm.verts.new(pt - n * 0.005))
        rows.append((row_o, row_i))
    faces = []
    m = len(prof)
    for (ao, ai), (bo, bi) in zip(rows, rows[1:]):
        for i in range(m - 1):
            faces.append(p.bm.faces.new((ao[i], ao[i + 1], bo[i + 1], bo[i])))
            faces.append(p.bm.faces.new((ai[i], bi[i], bi[i + 1], ai[i + 1])))
    for i in range(m - 1):
        for o, inn in (rows[0], rows[-1]):
            faces.append(p.bm.faces.new((o[i], inn[i], inn[i + 1], o[i + 1])))
    p._faces(faces, "Poly film", smooth=True)


def _row(p: Piece, across, a0, a1, z, width, height, along="y") -> None:
    """A row of crops as a ridge with a ragged top (for the stand-in), from
    a0 to a1 along y (or x), across from the middle: its leaves and ears
    are baked onto it."""
    n = 10
    top = [(a0 + (a1 - a0) * i / n, z + height * (0.75 + 0.25 * ((i * 7) % 3) / 2)) for i in range(n + 1)]
    outline = [(a0, z)] + top + [(a1, z)]
    if along == "y":
        # The outline's x is the row's y: turned a quarter so it runs along Y.
        p.prism(outline, width * 0.7, (across, 0, 0), "Leaf green", rot=(0, 0, 90))
    else:
        p.prism(outline, width * 0.7, (0, across, 0), "Leaf green")


def crop_bed(style: Style) -> Piece:
    """A raised bed of Fringe grain and greens, 4 × 1.2 m, boarded with scrap
    plank and old hull plate, with lean stalks, ears and leaves (see
    _plant)."""
    p = Piece("crop_bed", "prop", "4 × 1.2 m raised bed, 0.4 m high", budget=12000)
    r = rng(style, p.name)
    h = 0.4
    boards = []
    for x0, x1 in ((-2.0, -0.6), (-0.6, 0.7), (0.7, 2.0)):
        for y in (-0.6, 0.53):
            boards.append(((x0, y, 0), (x1, y + 0.07, h), r.choice(["Wood", "Wood dark"] + REPAINTS[:3])))
    for x in (-2.0, 1.93):
        boards.append(((x, -0.53, 0), (x + 0.07, 0.53, h), "Wood"))
    for lo, hi, mat in boards:
        p.span(lo, hi, mat)
    for x in (-2.0, -0.6, 0.7, 1.95):
        for y in (-0.62, 0.58):
            p.span((x - 0.03, y - 0.02, -0.05), (x + 0.03, y + 0.06, h + 0.06), "Wood dark")
    p.span((-1.93, -0.53, 0), (1.93, 0.53, h - 0.04), "Soil")
    _plants(p, (-1.8, -0.3), (1.8, -0.3), h - 0.04, r, "grain")
    _plants(p, (-1.8, 0.3), (1.8, 0.3), h - 0.04, r, "grain")
    _plants(p, (-1.7, 0.0), (1.7, 0.0), h - 0.04, r, "greens")
    with p.painted():
        p.stencil("4", (-1.6, -0.6 - 0.005, 0.2), 0.14, "Stencil white")
    p.collider((4.0, 1.2, h), (0, 0, h / 2))
    return p


def irrigation_pipe(style: Style) -> Piece:
    """4 m of the Fringe's irrigation: a patched pipe on low A-frame stands,
    coupled at each end to the next length, a valve, rag-and-tape repairs,
    and drip lines down to the soil. Too low and thin to collide with."""
    p = Piece("irrigation_pipe", "kit", "4 m pipe at 0.35 m; tiles along X; no collider")
    r = rng(style, p.name)
    z = 0.35
    p.tube([(-1.995, 0, z), (1.995, 0, z)], 0.05, "Sunbleached paint", segments=10)
    # Couplings at both ends: collars with bolts, half a coupling each.
    for x, s in ((-1.97, 1), (1.97, -1)):
        p.cyl(0.075, 0.06, (x + s * 0.03, 0, z), "Rust", rot=(0, 90, 0), segments=12)
        _bolts(p, [(x + s * 0.03, math.cos(a) * 0.065, z + math.sin(a) * 0.065) for a in (0, 2.1, 4.2)], 0.012, 0.075, "Steel",
               rot=(0, 90, 0))
    # A section replaced in other pipe, and a wrap of rag and tape.
    p.cyl(0.058, 0.7, (0.6, 0, z), "Repaint teal", rot=(0, 90, 0), segments=10)
    for x in (0.25, 0.95):
        p.cyl(0.068, 0.05, (x, 0, z), "Rust", rot=(0, 90, 0), segments=10)
    p.cyl(0.062, 0.22, (-1.0, 0, z), r.choice(["Fabric red", "Fabric ochre"]), rot=(0, 90, 0), segments=10)
    p.cyl(0.074, 0.05, (-0.94, 0, z), "Hull dark", rot=(0, 90, 0), segments=10)
    # A-frame stands, their feet in the dust.
    for x in (-1.4, 0.0, 1.4):
        for side in (-1, 1):
            p.tube([(x, side * 0.22, 0.0), (x, 0, z - 0.06)], 0.016, "Rust", segments=6)
        p.box((0.07, 0.14, 0.03), (x, 0, z - 0.065), "Rust")
        p.torus(0.055, 0.008, (x, 0, z), "Rust", rot=(0, 90, 0), segments=10, sides=4)
    # A gate valve with its wheel.
    p.cyl(0.07, 0.12, (-0.4, 0, z), "Hull dark", rot=(0, 90, 0), segments=10)
    p.cyl(0.025, 0.14, (-0.4, 0, z + 0.1), "Hull dark", segments=8)
    p.torus(0.07, 0.01, (-0.4, 0, z + 0.18), "Fabric red", segments=12, sides=4)
    for i in range(3):
        p.box((0.14, 0.01, 0.01), (-0.4, 0, z + 0.18), "Fabric red", rot=(0, 0, 60 * i))
    # Drip lines down to the beds.
    for i in range(7):
        x = -1.65 + i * 0.55
        p.cable((x, 0, z - 0.05), (x + 0.04, -0.12, 0.02), 0.006, "Rubber", sag=0.02, steps=4)
    # Laid by the dozen: the game draws a six-sided pipe and the stands as
    # thin prisms, with the couplings, valve and repairs baked onto them.
    with p.lowpoly():
        p.cyl(0.055, 3.99, (0, 0, z), "Sunbleached paint", rot=(0, 90, 0), segments=6)
        for x in (-1.4, 0.0, 1.4):
            p.prism([(-0.22, 0), (-0.19, 0), (0, z - 0.04), (0.19, 0), (0.22, 0), (0.02, z - 0.02), (-0.02, z - 0.02)],
                    0.03, (x, 0, 0), "Rust", rot=(0, 0, 90))
        p.span((-0.45, -0.07, z + 0.04), (-0.35, 0.07, z + 0.19), "Fabric red")
    return p


# Wrecks and salvage --------------------------------------------------------------

def wrecked_terraformer(style: Style) -> Piece:
    """One of Corvane's half-built terraformers, wrecked and half buried on
    the horizon: a machine hall panelled in Corvane's faded paint, its
    windows out and its doors jammed, cooling fins torn off its roof, and its
    stack snapped off and lying in the dust, plates torn from its broken
    end. 27 m end to end; you can climb it."""
    p = Piece("wrecked_terraformer", "prop", "about 27 × 10 m, 8.9 m high; climbable wreck", budget=120000)
    r = rng(style, p.name)
    # The machine hall, sunk a metre into the ground.
    hx, hy, top = 4.5, 3.5, 3.8
    p.span((-hx, -hy, -1.0), (hx, hy, top), "Corvane faded")
    # Panels round its walls, a centimetre proud, some gone to rust.
    for side in (-1, 1):
        for i in range(9):
            x0 = -hx + 0.08 + i * 1.0
            for z0, z1 in ((-0.28, 1.5), (1.55, 3.3)):
                mat = r.choices(["Corvane faded", "Rust", "Corvane faded"], [5, 3, 2])[0]
                if side < 0 and i in (3, 4, 5) and z0 > 0:
                    continue  # the window band
                p.span((x0, side * hy - (0.012 if side < 0 else 0), z0), (x0 + 0.94, side * hy + (0.012 if side > 0 else 0), z1), mat)
    for x in (-hx, hx):
        for j in range(7):
            y0 = -hy + 0.08 + j * 1.0
            p.span((x - (0.012 if x < 0 else 0), y0, -0.28), (x + (0.012 if x > 0 else 0), y0 + 0.94, 3.3),
                   r.choices(["Corvane faded", "Rust"], [3, 2])[0])
    # Ribs between the panels, and the roof's edge.
    for i in range(10):
        x = -hx + i * 1.0
        p.span((x - 0.05, -hy - 0.05, -0.98), (x + 0.05, -hy + 0.02, top - 0.02), "Rust")
        p.span((x - 0.05, hy - 0.02, -0.98), (x + 0.05, hy + 0.05, top - 0.02), "Rust")
    p.span((-hx - 0.1, -hy - 0.1, top - 0.05), (hx + 0.1, hy + 0.1, top + 0.25), "Hull dark")
    # The window band: a dark hall behind broken glass and twisted mullions.
    p.span((-1.5, -hy - 0.02, 1.6), (1.5, -hy + 0.3, 3.2), "Char")
    for i in range(7):
        x = -1.5 + i * 0.5
        p.box((0.06, 0.08, 1.6), (x, -hy - 0.03, 2.4), "Rust", rot=(0, r.uniform(-8, 8), 0))
    for _ in range(6):
        x = r.uniform(-1.3, 1.3)
        p.prism([(0, 0), (0.25, 0.05), (0.08, 0.4)], 0.01, (x, -hy - 0.05, r.uniform(1.65, 2.9)), "Glass",
                rot=(0, r.uniform(0, 360), 0))
    # A roller door jammed half up, the dark inside under it.
    p.span((-3.8, -hy - 0.02, -0.3), (-2.2, -hy + 0.4, 1.3), "Char")
    for i in range(8):
        p.span((-3.8, -hy - 0.06, 1.3 + i * 0.1), (-2.2, -hy - 0.02, 1.38 + i * 0.1), "Corvane faded")
    p.span((-3.95, -hy - 0.08, -0.32), (-3.8, -hy - 0.012, 2.2), "Hull dark")
    p.span((-2.2, -hy - 0.08, -0.32), (-2.05, -hy - 0.012, 2.2), "Hull dark")
    # Vents with louvres between the ribs, rust streaking down from them.
    for x in (2.0, 3.0):
        p.span((x - 0.44, -hy - 0.052, 1.8), (x + 0.44, -hy - 0.012, 2.8), "Hull dark")
        for i in range(6):
            p.box((0.84, 0.05, 0.06), (x, -hy - 0.06, 1.9 + i * 0.16), "Rust", rot=(-30, 0, 0))
    with p.painted():
        p.stencil("07", (-3.0, -hy - 0.018, 2.4), 0.8, "Corvane blue")
        p.span((-hx, -hy - 0.02, 3.35), (hx, -hy - 0.015, 3.5), "Corvane blue")
        for x in (2.0, 3.0):
            p.span((x - 0.15, -hy - 0.02, -0.3), (x + 0.1, -hy - 0.015, 1.8), "Rust")
        p.stencil("90", (3.2, hy + 0.018, 2.4), 0.8, "Corvane blue", facing="+Y")
    # The roof: the cooling fin bank, bent and torn, pipes along it.
    p.span((-4.2, -3.0, top + 0.25), (-0.6, 3.0, top + 0.7), "Hull dark")
    for i in range(14):
        x = -4.05 + i * 0.26
        if r.random() < 0.15:
            continue
        tall = 1.7 if r.random() > 0.3 else r.uniform(0.4, 1.1)
        p.box((0.05, 5.6, tall), (x, 0, top + 0.7 + tall / 2), r.choice(["Rust", "Corvane faded", "Rust"]),
              rot=(r.uniform(-6, 6), r.uniform(-10, 10), 0))
    for y in (-3.2, 3.2):
        p.tube([(-4.3, y, top + 0.45), (0.0, y, top + 0.45), (0.6, y * 0.7, top + 0.45)], 0.12, "Rust", segments=10)
        for x in (-3.5, -2.0, -0.5):
            p.cyl(0.17, 0.08, (x, y, top + 0.45), "Hull dark", rot=(0, 90, 0), segments=12)
    # Broken handrail round the roof.
    for i in range(9):
        x = -4.3 + i * 1.07
        p.cyl(0.025, 1.0, (x, -hy + 0.1, top + 0.75), "Rust", segments=6)
    p.tube([(-4.3, -hy + 0.1, top + 1.25), (-0.5, -hy + 0.1, top + 1.25), (0.4, -hy - 0.4, top + 0.6)], 0.025, "Rust", segments=6)
    # The stump of the stack, snapped off above the roof.
    sx = 2.4
    p.cyl(2.0, 2.6, (sx, 0, top + 1.3), "Corvane faded", segments=20)
    for z in (top + 0.3, top + 1.4):
        p.torus(2.05, 0.08, (sx, 0, z), "Rust", segments=20, sides=6)
    p.cyl(1.9, 0.05, (sx, 0, top + 2.58), "Char", segments=20)
    for i in range(12):
        a = math.tau * i / 12
        tall = r.uniform(0.3, 1.3)
        p.prism([(-0.4, 0), (0.4, 0), (0.12, tall), (-0.1, tall * 0.65)], 0.07,
                (sx + math.cos(a) * 1.97, math.sin(a) * 1.97, top + 2.55), r.choice(["Corvane faded", "Rust"]),
                rot=(r.uniform(-12, 12), 0, math.degrees(a) + 90))
    # The ladder still bolted up the hall's flank.
    for x in (-4.1, -3.6):
        p.span((x - 0.03, -hy - 0.14, -0.2), (x + 0.03, -hy - 0.08, top + 1.0), "Rust")
    for i in range(14):
        p.span((-4.1, -hy - 0.15, 0.3 + i * 0.3), (-3.6, -hy - 0.09, 0.335 + i * 0.3), "Rust")
    # The stack itself, lying where it fell: tilted into the ground, its
    # sections flanged together and ringed with stiffeners.
    tilt = 96
    axis = Vector((math.sin(math.radians(tilt)), 0, math.cos(math.radians(tilt))))
    start = Vector((5.2, 0.6, 2.6))
    length, radius = 15.0, 2.2
    mats = ["Rust", "Corvane faded", "Rust", "Corvane faded", "Rust"]
    for i, mat in enumerate(mats):
        c = start + axis * (length * (i + 0.5) / len(mats))
        p.cyl(radius, length / len(mats) - 0.04, c, mat, rot=(0, tilt, 0), segments=28)
        flange = start + axis * (length * i / len(mats))
        p.torus(radius + 0.06, 0.12, flange, "Hull dark", rot=(0, tilt, 0), segments=28, sides=6)
        p.torus(radius + 0.03, 0.06, c, "Rust", rot=(0, tilt, 0), segments=28, sides=4)
    with p.painted():
        for i in (1, 3):
            c = start + axis * (length * (i + 0.5) / len(mats))
            p.stencil("07", c + Vector((0, -radius - 0.006, 0.4)), 0.9, "Corvane blue")
    # Its broken end: torn plating, the dark inside, ribs showing.
    end = start + axis * length
    p.cyl(radius - 0.12, 0.3, end - axis * 0.2, "Char", rot=(0, tilt, 0), segments=28)
    for i in range(9):
        a = math.tau * i / 9 + 0.3
        off = Vector((0, math.cos(a), math.sin(a))) * (radius - 0.04)
        tall = r.uniform(0.5, 1.3)
        p.prism([(-0.45, 0), (0.45, 0), (0.18, tall), (-0.25, tall * 0.7)], 0.06, end + off, "Rust",
                rot=(math.degrees(a) - 90, tilt + r.uniform(-15, 15), 0))
    p.torus(radius - 0.1, 0.06, end - axis * 0.6, "Rust", rot=(0, tilt, 0), segments=24, sides=4)
    # Struts, hanging cables and plates strewn about.
    for y in (-3.6, 3.6):
        p.tube([(4.4, y, 3.5), (6.5, y * 1.3, -0.2)], 0.12, "Rust", segments=8)
        p.cyl(0.2, 0.1, (4.4, y, 3.5), "Hull dark", rot=(90, 0, 0), segments=10)
    p.cable((4.4, -2.0, 3.7), (5.8, -2.9, 0.1), 0.05, "Rubber", sag=0.8, segments=6)
    p.cable((4.4, -1.4, 3.7), (6.4, -2.2, 0.1), 0.035, "Rubber", sag=1.0, segments=6)
    for _ in range(8):
        x, y = r.uniform(-6, 20), r.choice((-1, 1)) * r.uniform(3.8, 5.0)
        p.prism([(-0.5, 0), (0.6, 0), (0.4, 0.7), (-0.3, 0.5)], 0.05, (x, y, 0.05), r.choice(["Rust", "Corvane faded"]),
                rot=(r.uniform(-80, -60), 0, r.uniform(0, 360)))
    # Drifts of red dust banked against it all.
    for at, rx, ry, h in (((-4.9, 0, 0), 1.3, 3.8, 1.1), ((0, -3.9, 0), 4.2, 1.1, 0.8), ((12.0, -2.4, 0), 6.0, 1.3, 1.0),
                          ((17.5, 1.0, 0), 3.0, 3.2, 1.4), ((1.0, 3.9, 0), 3.6, 1.0, 0.9)):
        _drift(p, at, rx, ry, h, "Dust red", r, rings=6, segments=24, lumps=3)
    p.collider((9.0, 7.0, 3.8), (0, 0, 1.9))
    p.collider((3.6, 6.0, 1.0), (-2.4, 0, 4.3))
    p.collider((3.0, 3.0, 2.4), (sx, 0, 5.0))
    p.collider((3.2, 3.2, length), start + axis * (length / 2), rot=(0, tilt, 0))
    return p


def terraformer_debris(style: Style) -> Piece:
    """A broken length of terraformer stack, 4 m of it, tipped on its side:
    a flanged section, stiffener rings, torn plates round its broken end, a
    bank of fins still hanging off it, and the dust drifted up against it."""
    p = Piece("terraformer_debris", "prop", "about 4.4 × 4.7 m, 2.7 m high")
    r = rng(style, p.name)
    yaw = 18
    rot = (0, 90, yaw)
    along = Vector((math.cos(math.radians(yaw)), math.sin(math.radians(yaw)), 0))
    c = Vector((0, 0, 1.25))
    p.cyl(1.3, 3.2, c, "Rust", rot=rot, segments=24)
    p.cyl(1.32, 1.2, c - along * 0.6, "Corvane faded", rot=rot, segments=24)
    for d in (-1.6, -0.3, 0.9):
        p.torus(1.36, 0.07, c + along * d, "Hull dark", rot=rot, segments=24, sides=6)
    _bolts(p, [c - along * 1.6 + Vector((0, 0, 0)) + Vector((-math.sin(math.radians(yaw)), math.cos(math.radians(yaw)), 0)) * math.cos(a) * 1.42
               + Vector((0, 0, math.sin(a) * 1.42)) for a in (math.tau * i / 12 for i in range(12))], 0.03, 0.06, "Steel")
    end = c + along * 1.6
    p.cyl(1.2, 0.2, end - along * 0.15, "Char", rot=rot, segments=24)
    for i in range(8):
        a = math.tau * i / 8
        off = Vector((-math.sin(math.radians(yaw)) * math.cos(a), math.cos(math.radians(yaw)) * math.cos(a), math.sin(a))) * 1.25
        tall = r.uniform(0.4, 0.9)
        p.prism([(-0.3, 0), (0.3, 0), (0.1, tall), (-0.18, tall * 0.6)], 0.05, end + off, "Rust",
                rot=(math.degrees(a) - 90, 90 + r.uniform(-15, 15), yaw))
    # A bank of fins, bent, still bolted to a strip of plate.
    p.box((2.0, 0.15, 0.6), (-0.2, -1.55, 0.3), "Hull dark", rot=(0, 0, yaw))
    for i in range(8):
        x = -1.0 + i * 0.25
        p.box((0.05, 1.4, 0.95), (x * math.cos(math.radians(yaw)) + 0.45, x * math.sin(math.radians(yaw)) - 1.6, 0.55),
              r.choice(["Rust", "Hull dark", "Corvane faded"]), rot=(r.uniform(-12, 12), 0, yaw + r.uniform(-8, 8)))
    _drift(p, (0.3, 1.3, 0), 2.1, 0.9, 0.7, "Dust red", r, rings=5, segments=18)
    _drift(p, (-1.6, -0.3, 0), 0.9, 1.5, 0.55, "Dust red", r, rings=4, segments=14)
    p.collider((3.2, 2.0, 2.0), (0, 0, 1.0), rot=(0, 0, yaw))
    return p


def scrap_pile(style: Style) -> Piece:
    """A salvage heap picked over by every crew that's passed: plates bent
    and stacked, pipe, a drum, a wheel and a tyre, a rusted spade left
    standing in it, all on a mound of dust and spoil."""
    p = Piece("scrap_pile", "prop", "about 3.1 × 2.5 m heap, 1.3 m high: mantle")
    r = rng(style, p.name)
    _drift(p, (0, 0, 0), 1.5, 1.2, 0.8, "Dust red", r, rings=6, segments=22, lumps=3)
    _drift(p, (0.2, 0.1, 0), 0.95, 0.75, 1.0, "Rock dark", r, rings=5, segments=18, lumps=2)

    def height(x, y):
        return 0.95 * max(0.0, 1 - (x / 1.4) ** 2 - (y / 1.1) ** 2) ** 0.6

    for _ in range(14):
        x, y = r.uniform(-1.0, 1.0), r.uniform(-0.8, 0.8)
        z = height(x, y)
        mat = r.choice(REPAINTS + ["Rust", "Rust", "Hull alloy"])
        if r.random() < 0.6:
            # A bent plate: two leaves at an angle.
            w, d = r.uniform(0.4, 0.8), r.uniform(0.3, 0.55)
            yaw = r.uniform(0, 180)
            p.box((w, d, 0.03), (x, y, z), mat, rot=(r.uniform(-30, 30), r.uniform(-25, 25), yaw))
            p.box((w * 0.6, d, 0.03), (x + 0.2 * math.cos(math.radians(yaw)), y + 0.2 * math.sin(math.radians(yaw)), z + 0.12),
                  mat, rot=(r.uniform(-30, 30), r.uniform(30, 55), yaw))
        else:
            a = r.uniform(0, math.tau)
            dvec = Vector((math.cos(a), math.sin(a), r.uniform(-0.3, 0.4))) * r.uniform(0.5, 0.9)
            p.tube([(x, y, z), (x + dvec.x, y + dvec.y, z + dvec.z)], r.uniform(0.03, 0.06), mat, segments=8)
    p.lathe([(0.0, 0.0), (0.27, 0.0), (0.29, 0.03), (0.29, 0.83), (0.27, 0.86), (0, 0.86)], (-0.8, 0.5, 0.25), "Rust",
            rot=(80, 0, 30), segments=16)
    p.prism([(-0.5, 0), (0.6, 0), (0.4, 0.55), (-0.3, 0.7)], 0.05, (0.2, 0.3, 0.75), "Corvane faded", rot=(-25, -20, 35))
    p.source("old_tyre", at=(0.75, -0.65, 0.2), rot=(70, 0, 20), height=0.6)
    p.source("rusted_wheel_rim_01", at=(-0.4, -0.75, 0.15), rot=(60, 0, -30), height=0.42)
    p.source("can_rusted", at=(1.2, 0.5, 0.0), height=0.18)
    p.source("can_rusted", at=(-1.25, -0.2, 0.05), rot=(90, 0, 40), height=0.11)
    p.source("rusted_spade_01", at=(0.35, 0.25, 0.55), rot=(0, 18, 25), height=1.1)
    p.collider((2.2, 1.7, 1.1), (0, 0, 0.55))
    return p


def salvage_frame(style: Style) -> Piece:
    """A Fringer salvage dig: a lashed timber A-frame over a pit shored with
    planks, a pulley block and a hand winch with its ratchet, a bucket on the
    rope, a sieve on legs and the spoil heaped beside with what's come up."""
    p = Piece("salvage_frame", "prop", "about 5 × 3 m dig, A-frame 3.4 m high; no collider")
    r = rng(style, p.name)
    top = 3.0
    # The A-frames: poles lashed at the top and braced.
    for x in (-1.5, 1.5):
        for y in (-0.9, 0.9):
            p.tube([(x, y * 1.05, -0.05), (x, 0, top)], 0.065, r.choice(["Wood", "Wood dark"]), segments=8)
        p.tube([(x, -0.55, 1.15), (x, 0.55, 1.15)], 0.04, "Wood", segments=8)
        for k in range(4):
            p.torus(0.085, 0.012, (x, 0, top - 0.12 - k * 0.035), "Rope", rot=(90, 0, 0), segments=10, sides=4)
    p.tube([(-1.7, 0, top + 0.05), (1.7, 0, top + 0.05)], 0.08, "Rust", segments=10)
    # The pulley block, hung off the beam.
    p.span((-0.04, -0.06, top - 0.32), (0.04, 0.06, top), "Hull dark")
    p.cyl(0.16, 0.06, (0, 0, top - 0.2), "Steel", rot=(90, 0, 0), segments=16)
    p.cyl(0.04, 0.14, (0, 0, top - 0.2), "Hull dark", rot=(90, 0, 0), segments=8)
    # The pit, shored with planks round its rim, its dark depth.
    p.cyl(1.05, 0.02, (0, 0, 0.0), "Char", segments=24)
    for i in range(10):
        a = math.tau * i / 10
        p.box((0.62, 0.05, 0.3), (math.cos(a) * 1.08, math.sin(a) * 1.08, 0.05), "Wood dark", rot=(0, 0, math.degrees(a) + 90))
    for i in range(12):
        a = math.tau * i / 12
        p.sphere(0.2, (math.cos(a) * 1.3, math.sin(a) * 1.3, 0.0), "Dust red", scale=(1.4, 1.0, 0.6),
                 rot=(0, 0, math.degrees(a) + 90), segments=10, rings=5)
    # The rope down into it, and the bucket on it.
    p.cable((0.16, 0, top - 0.2), (0.16, 0, 0.95), 0.012, "Rope", steps=2)
    p.lathe([(0.0, 0.62), (0.15, 0.62), (0.2, 0.92), (0.19, 0.93), (0.14, 0.64), (0, 0.64)], (0.16, 0, 0), "Hull alloy",
            segments=14)
    p.torus(0.2, 0.012, (0.16, 0, 0.92), "Hull dark", segments=14, sides=4)
    p.torus(0.2, 0.008, (0.16, 0, 0.93), "Hull dark", rot=(90, 0, 0), segments=12, sides=4, arc=180)
    # The hand winch on the left legs: a drum, a ratchet gear, a crank.
    wx, wy, wz = -1.5, -0.25, 1.0
    p.cyl(0.12, 0.42, (wx, wy, wz), "Hull dark", rot=(90, 0, 0), segments=14)
    p.cyl(0.13, 0.28, (wx, wy, wz), "Rope", rot=(90, 0, 0), segments=14)
    teeth = []
    for i in range(20):
        a = math.tau * i / 20
        rr = 0.2 if i % 2 == 0 else 0.17
        teeth.append((math.cos(a) * rr, math.sin(a) * rr))
    p.prism(teeth, 0.03, (wx, wy - 0.24, wz), "Steel", rot=(0, 0, 0))
    p.tube([(wx, wy - 0.27, wz), (wx, wy - 0.4, wz), (wx + 0.25, wy - 0.4, wz + 0.15), (wx + 0.25, wy - 0.55, wz + 0.15)],
           0.018, "Crew orange", segments=6)
    p.cable((wx + 0.05, wy, wz + 0.13), (-0.16, 0, top - 0.2), 0.012, "Rope", sag=0.05)
    # A sieve on legs, and the spoil with what's come up out of the dig.
    sx, sy = 2.1, -1.0
    p.span((sx - 0.5, sy - 0.35, 0.75), (sx + 0.5, sy + 0.35, 0.82), "Wood")
    with p.painted():
        p.span((sx - 0.44, sy - 0.29, 0.825), (sx + 0.44, sy + 0.29, 0.83), "Hull dark")
    for dx in (-0.45, 0.45):
        for dy in (-0.3, 0.3):
            p.tube([(sx + dx, sy + dy, 0), (sx + dx * 0.9, sy + dy * 0.9, 0.76)], 0.025, "Wood dark", segments=6)
    _drift(p, (2.6, 0.7, 0), 1.0, 0.85, 0.6, "Dust red", r, rings=5, segments=18)
    p.box((0.6, 0.4, 0.04), (2.3, 0.3, 0.55), "Corvane faded", rot=(12, 20, 30))
    p.lathe([(0, 0), (0.17, 0), (0.19, 0.05), (0.19, 0.45), (0.17, 0.48), (0, 0.48)], (1.6, 1.3, 0.2), "Rust",
            rot=(80, 0, 40), segments=12)
    p.source("rusted_spade_01", at=(2.85, 1.15, 0.35), rot=(0, -15, 60), height=1.1)
    return p


def caravan_cart(style: Style) -> Piece:
    """A Fringer's hand cart, the caravans' workhorse: two big spoked wheels
    with iron tyres, a planked bed with stake sides, shafts to pull it by,
    and a load of crates and sacks under a sun-bleached tarp, roped down,
    with a jerrycan of water hung off the back."""
    p = Piece("caravan_cart", "prop", "1.8 × 3.6 m cart, load 1.5 m high")
    r = rng(style, p.name)
    bed = 0.62
    # The bed: planks with gaps, cross-bearers under, stake sides.
    for i in range(7):
        x = -0.66 + i * 0.22
        p.span((x - 0.1, -1.2, bed), (x + 0.1, 1.2, bed + 0.05), r.choice(["Wood", "Wood", "Wood dark"]))
    for y in (-1.0, 0.0, 1.0):
        p.span((-0.72, y - 0.05, bed - 0.08), (0.72, y + 0.05, bed), "Wood dark")
    for x in (-0.72, 0.66):
        for z0 in (bed + 0.12, bed + 0.3):
            p.span((x, -1.2, z0), (x + 0.06, 1.2, z0 + 0.12), "Wood")
        for y in (-1.15, -0.4, 0.4, 1.15):
            p.span((x - 0.005, y - 0.03, bed + 0.05), (x + 0.065, y + 0.03, bed + 0.45), "Wood dark")
    p.span((-0.72, 1.14, bed + 0.05), (0.72, 1.2, bed + 0.4), "Wood dark")
    p.span((-0.8, -0.06, bed - 0.16), (0.8, 0.06, bed - 0.08), "Rust")
    # Wheels: iron tyre, wooden felloe, twelve spokes, hub.
    for x in (-0.86, 0.86):
        p.torus(0.6, 0.035, (x, 0, 0.62), "Rust", rot=(0, 90, 0), segments=24, sides=6)
        p.torus(0.55, 0.045, (x, 0, 0.62), "Wood dark", rot=(0, 90, 0), segments=24, sides=6)
        for i in range(12):
            p.box((0.035, 0.05, 1.04), (x, 0, 0.62), "Wood", rot=(180 * i / 12, 0, 0))
        p.cyl(0.11, 0.2, (x, 0, 0.62), "Wood dark", rot=(0, 90, 0), segments=12)
        p.cyl(0.05, 0.26, (x, 0, 0.62), "Rust", rot=(0, 90, 0), segments=8)
    # Shafts out front, a crossbar to pull on, a prop leg.
    for x in (-0.55, 0.55):
        p.tube([(x, -1.1, bed + 0.02), (x * 0.95, -2.4, 0.92)], 0.04, "Wood", segments=8)
        p.torus(0.045, 0.012, (x * 0.97, -1.8, 0.78), "Rust", rot=(90, 0, 0), segments=8, sides=4)
    p.tube([(-0.6, -2.35, 0.92), (0.6, -2.35, 0.92)], 0.035, "Wood dark", segments=8)
    p.tube([(0, -1.35, bed), (0, -1.45, 0)], 0.03, "Wood dark", segments=6)
    # The load: crates and sacks.
    for (x, y), size in (((-0.32, -0.6), (0.6, 0.6, 0.45)), ((0.3, 0.45), (0.55, 0.6, 0.5)), ((0.3, -0.35), (0.5, 0.45, 0.35))):
        mat = r.choice(REPAINTS)
        p.box(size, (x, y, bed + 0.05 + size[2] / 2), mat)
        p.box((size[0] + 0.02, 0.05, size[2] + 0.02), (x, y, bed + 0.05 + size[2] / 2), "Wood dark")
    for x, y in ((-0.35, 0.45), (-0.3, 0.0)):
        p.sphere(0.25, (x, y, bed + 0.28), "Fabric sand", scale=(1, 1.3, 0.75), rot=(0, 0, 20), segments=12, rings=8)
    # The tarp over it all, draped, and the ropes holding it.
    lift, y0, y1 = 1.05, -1.15, 1.2
    p.cloth([(-0.8, y0, bed + 0.15), (0.8, y0, bed + 0.15), (0.8, y1, bed + 0.15), (-0.8, y1, bed + 0.15)],
            "Canvas", sag=-lift, ripple=0.03, droop=(0.2, 1, 0.2, 1), cell=0.08)
    # Ropes over it, following its shape (the cloth's own droop, inverted).
    for y in (-0.7, 0.0, 0.7):
        v = (y - y0) / (y1 - y0)
        pts = []
        for i in range(13):
            u = i / 12
            z = bed + 0.15 + lift / 2 * (0.2 * 4 * u * (1 - u) + 4 * v * (1 - v)) + 0.02
            pts.append((-0.8 + 1.6 * u, y, z))
        pts = [(-0.82, y, bed + 0.1)] + pts + [(0.82, y, bed + 0.1)]
        p.tube(pts, 0.012, "Rope", segments=5)
    p.source("metal_jerrycan_green", at=(0.45, 1.32, 0.25), rot=(0, 0, 90), height=0.5)
    p.collider((1.4, 2.4, 1.3), (0, 0, 0.65))
    return p


def fringer_tent(style: Style) -> Piece:
    """A Fringer's ridge tent: bleached canvas sagging between its poles,
    patched, red with dust along its skirts, its door flaps tied back on a
    bedroll and a lantern, guy ropes pegged out."""
    p = Piece("fringer_tent", "prop", "2.6 × 2.9 m tent, 1.95 m high")
    r = rng(style, p.name)
    hw, ridge, half = 1.2, 1.9, 1.4
    # Poles and the ridge pole.
    for y in (-half - 0.05, half + 0.05):
        p.cyl(0.03, ridge + 0.1, (0, y, (ridge + 0.1) / 2), "Wood", segments=8)
        p.sphere(0.04, (0, y, ridge + 0.11), "Rust", segments=8, rings=5)
    p.tube([(0, -half - 0.05, ridge + 0.02), (0, half + 0.05, ridge + 0.02)], 0.028, "Wood", segments=8)
    # The two sides of the fly, sagging between poles, staked out low.
    for side in (-1, 1):
        p.cloth([(0, -half, ridge), (0, half, ridge), (side * hw, half, 0.05), (side * hw, -half, 0.05)],
                "Canvas", sag=0.07, ripple=0.012, droop=(0, 0.6, 0.3, 0.6), seed=side + 3)
    # The back wall, and the door flaps folded back.
    p.prism([(-hw + 0.02, 0.05), (hw - 0.02, 0.05), (0, ridge - 0.02)], 0.02, (0, half - 0.02, 0), "Canvas")
    for side in (-1, 1):
        p.prism([(0, 0.05), (side * 0.75, 0.05), (side * 0.12, ridge - 0.1)], 0.02, (side * 0.15, -half - 0.08, 0), "Canvas",
                rot=(0, 0, side * 40))
        p.torus(0.07, 0.012, (side * 0.62, -half - 0.25, 1.0), "Rope", rot=(0, 90, 0), segments=8, sides=4)
    with p.painted():
        # Patches, and the dust red up its skirts.
        for side in (-1, 1):
            nrm = Vector((side * ridge, 0, hw)).normalized()
            for _ in range(2):
                f = r.uniform(0.3, 0.7)
                y = r.uniform(-0.9, 0.9)
                c = Vector((side * hw * f, y, ridge * (1 - f) + 0.05 * f)) + nrm * 0.012
                p.box((0.38, 0.45, 0.006), c, r.choice(["Fabric ochre", "Fabric teal", "Fabric red"]),
                      rot=(0, side * math.degrees(math.atan2(ridge, hw)), 0))
            c = Vector((side * hw * 0.9, 0, 0.2)) + nrm * 0.01
            p.box((0.32, 2 * half - 0.05, 0.006), c, "Dust red", rot=(0, side * math.degrees(math.atan2(ridge, hw)), 0))
    # Guy ropes out to pegs, front and back and along the sides.
    for y, out in ((-half - 0.05, -2.3), (half + 0.05, 2.3)):
        p.cable((0, y, ridge), (0, out, 0.05), 0.007, "Rope", sag=0.05, steps=6)
        p.prism([(-0.02, 0), (0.02, 0), (0.03, 0.22), (-0.03, 0.22)], 0.03, (0, out, -0.1), "Wood dark")
    for side in (-1, 1):
        for y in (-0.8, 0.8):
            a = Vector((side * hw * 0.55, y, ridge * 0.45))
            b = Vector((side * 2.1, y * 1.1, 0.05))
            p.cable(a, b, 0.006, "Rope", sag=0.04, steps=6)
            p.prism([(-0.02, 0), (0.02, 0), (0.03, 0.2), (-0.03, 0.2)], 0.03, b - Vector((0, 0, 0.12)), "Wood dark")
    # A ground sheet, a bedroll, a kit bag and a lantern by the door.
    p.span((-1.05, -1.3, 0), (1.05, 1.3, 0.02), "Fabric red")
    p.cyl(0.13, 0.75, (-0.55, 0.4, 0.15), "Fabric indigo", rot=(0, 90, 10), segments=12)
    p.sphere(0.22, (0.5, 0.6, 0.2), "Fabric olive", scale=(1, 0.8, 0.9), segments=12, rings=8)
    p.source("wooden_lantern_01", at=(0.45, -half - 0.35, 0), height=0.42)
    p.collider((2.0, 2.6, 1.5), (0, 0, 0.75))
    return p


def camp_stove(style: Style) -> Piece:
    """A camp fire in a ring of stones, a grate over the embers with a pot on
    it and a kettle by it: the middle of every Fringe camp."""
    p = Piece("camp_stove", "prop", "0.95 m fire ring, pot to 0.55 m")
    r = rng(style, p.name)
    p.source("stone_fire_pit", at=(0, 0, 0), length=0.95, recolour=ROCK)
    p.cyl(0.3, 0.03, (0, 0, 0.03), "Char", segments=14)
    # Logs, charred, and embers among them.
    for i in range(4):
        a = math.tau * i / 4 + 0.4
        p.cyl(0.045, 0.5, (math.cos(a) * 0.08, math.sin(a) * 0.08, 0.1), "Wood dark", rot=(80, 0, math.degrees(a) + 90), segments=8)
        p.cyl(0.046, 0.02, (math.cos(a) * 0.32, math.sin(a) * 0.32, 0.1), "Char", rot=(80, 0, math.degrees(a) + 90), segments=8)
    for i in range(6):
        a = math.tau * i / 6
        p.lathe([(0.0, 0.0), (0.05, 0.0), (0.0, 0.11)], (math.cos(a) * 0.13, math.sin(a) * 0.13, 0.06), "Ember", segments=6)
    # The grate, on its own legs, and the pot on it.
    for i in range(6):
        y = -0.18 + i * 0.072
        p.cyl(0.008, 0.66, (0, y, 0.27), "Hull dark", rot=(0, 90, 0), segments=6)
    for x in (-0.3, 0.3):
        p.span((x - 0.012, -0.21, 0.255), (x + 0.012, 0.21, 0.27), "Hull dark")
        for y in (-0.2, 0.2):
            p.cyl(0.01, 0.27, (x, y, 0.135), "Hull dark", segments=6)
    p.source("pot_enamel_01", at=(0.06, 0, 0.28), height=0.18)
    p.lathe([(0, 0), (0.08, 0), (0.1, 0.1), (0.07, 0.17), (0.02, 0.2), (0, 0.2)], (-0.55, 0.3, 0.0), "Repaint teal", segments=14)
    p.cyl(0.012, 0.12, (-0.47, 0.3, 0.12), "Repaint teal", rot=(0, 60, 0), segments=6)
    p.torus(0.06, 0.008, (-0.55, 0.3, 0.22), "Hull dark", rot=(90, 0, 0), segments=10, sides=4, arc=180)
    p.collider((0.85, 0.85, 0.4), (0, 0, 0.2))
    return p


def water_tank_fringe(style: Style) -> Piece:
    """A Fringe cistern: a riveted tank patched over many times, banded, on a
    braced stand, a ladder up to its hatch, an inlet from the roof gutter, a
    gauge, and a tap with a bucket under it. Water's worth more than marks
    out here."""
    p = Piece("water_tank_fringe", "prop", "2.1 × 2.4 m tank on a 1.2 m stand, 3.45 m high")
    r = rng(style, p.name)
    stand = 1.2
    # The stand: angle-iron legs, braced both ways, a deck of planks.
    for x in (-0.72, 0.72):
        for y in (-0.72, 0.72):
            p.span((x - 0.05, y - 0.05, 0), (x + 0.05, y + 0.05, stand), "Rust")
            p.span((x - 0.09, y - 0.09, 0), (x + 0.09, y + 0.09, 0.02), "Rust")
    for a, b in (((-0.72, -0.72), (0.72, -0.72)), ((-0.72, 0.72), (0.72, 0.72)), ((-0.72, -0.72), (-0.72, 0.72)), ((0.72, -0.72), (0.72, 0.72))):
        p.tube([(a[0], a[1], 0.15), (b[0], b[1], stand - 0.1)], 0.02, "Rust", segments=6)
        p.tube([(b[0], b[1], 0.15), (a[0], a[1], stand - 0.1)], 0.02, "Rust", segments=6)
    for i in range(7):
        y = -0.78 + i * 0.26
        p.span((-0.86, y, stand), (0.86, y + 0.24, stand + 0.06), r.choice(["Wood", "Wood dark"]))
    # The tank: riveted courses, bands, a domed lid and its hatch.
    p.lathe([(0, stand + 0.06), (0.97, stand + 0.06), (1.0, stand + 0.16), (1.0, 3.0), (0.93, 3.15), (0.3, 3.33), (0, 3.35)],
            (0, 0, 0), "Repaint teal", segments=14)
    for z in (1.65, 2.25, 2.85):
        p.torus(1.012, 0.02, (0, 0, z), "Hull dark", segments=14, sides=4)
        _bolts(p, [(math.cos(a) * 1.005, math.sin(a) * 1.005, z + 0.04) for a in (math.tau * i / 14 for i in range(14))], 0.008,
               0.02, "Hull dark", rot=(0, 0, 0))
    for z in (1.4, 2.6):
        p.torus(1.02, 0.03, (0, 0, z), "Rust", segments=14, sides=4)
    p.cyl(0.22, 0.1, (0.2, 0.2, 3.32), "Hull dark", segments=12)
    p.cyl(0.2, 0.04, (0.2, 0.2, 3.39), r.choice(REPAINTS), segments=12)
    # Patches welded on, a centimetre proud.
    for _ in range(5):
        a = r.uniform(0, math.tau)
        z = r.uniform(1.5, 2.9)
        p.box((r.uniform(0.3, 0.55), 0.012, r.uniform(0.25, 0.45)), (math.cos(a) * 1.012, math.sin(a) * 1.012, z),
              r.choice(REPAINTS + ["Rust"]), rot=(0, 0, math.degrees(a) + 90))
    with p.painted():
        p.lathe([(1.006, stand + 0.17), (1.006, 1.85)], (0, 0, 0), "Dust red", segments=14)
        p.stencil("12", (0.0, -1.008, 2.55), 0.3, "Stencil white")
    # The ladder up the +X side to the hatch.
    for y in (-0.2, 0.2):
        p.tube([(1.1, y, stand + 0.06), (1.1, y, 3.45), (0.85, y * 0.8, 3.5)], 0.02, "Rust", segments=6)
    for i in range(7):
        p.box((0.025, 0.4, 0.025), (1.1, 0, stand + 0.3 + i * 0.3), "Rust")
    # The inlet down from a gutter, and a gauge.
    p.tube([(-0.6, 0.6, 3.25), (-0.9, 0.9, 3.6)], 0.05, "Hull alloy", segments=8)
    p.cyl(0.08, 0.04, (-0.72, -0.72, 2.0), "Hull dark", rot=(90, 0, 45), segments=12)
    p.cyl(0.065, 0.06, (-0.72, -0.72, 2.0), "Paper", rot=(90, 0, 45), segments=12)
    # The tap: down off the tank's foot, a valve wheel, a bucket under it.
    p.tube([(0, -0.95, stand + 0.3), (0, -1.15, stand + 0.3), (0, -1.15, 0.7)], 0.035, "Hull alloy", segments=8)
    p.cyl(0.05, 0.08, (0, -1.15, 0.92), "Hull dark", segments=8)
    p.torus(0.07, 0.01, (0, -1.15, 0.99), "Fabric red", segments=12, sides=4)
    p.lathe([(0.0, 0.0), (0.13, 0.0), (0.17, 0.3), (0.165, 0.3), (0.125, 0.015), (0, 0.015)], (0, -1.15, 0), "Hull alloy",
            segments=14)
    p.torus(0.17, 0.008, (0, -1.15, 0.3), "Hull dark", segments=12, sides=4)
    p.collider((1.8, 1.8, 3.3), (0, 0, 1.65))
    return p


# Fences and signs ---------------------------------------------------------------

def wire_fence(style: Style) -> Piece:
    """4 m of Fringe fence: a post of pipe and a post of plank, four strands
    of barbed wire sagging between them, twisted at the posts, and rags tied
    on so it can be seen in a storm. Tiles along X; the game draws a few
    dozen triangles, the barbs and twists baked onto them."""
    p = Piece("wire_fence", "kit", "4 m fence, 1.35 m high: tiles along X", budget=500)
    r = rng(style, p.name)
    strands = (0.3, 0.6, 0.9, 1.15)
    sags = [r.uniform(0.03, 0.07) for _ in strands]
    # The posts: a pipe at -2, a plank at 0.
    p.cyl(0.05, 1.35, (-2.0, 0, 0.675), "Rust", segments=10)
    p.cyl(0.055, 0.03, (-2.0, 0, 1.35), "Hull dark", segments=10)
    p.span((-0.05, -0.05, 0), (0.05, 0.05, 1.3), "Wood")
    for z in strands:
        for x in (-2.0, 0.0):
            p.torus(0.06, 0.006, (x, 0, z), "Steel", rot=(0, 0, 0), segments=8, sides=4)
    for z, sag in zip(strands, sags):
        for x0 in (-2.0, 0.0):
            p.cable((x0, -0.06, z), (x0 + 2.0, -0.06, z), 0.004, "Steel", sag=sag, steps=8, segments=4)
            for i in range(1, 8):
                x = x0 + i * 0.25
                zz = z - sag * 4 * (i / 8) * (1 - i / 8)
                p.box((0.004, 0.05, 0.004), (x, -0.06, zz), "Steel", rot=(45, 0, 30))
                p.box((0.004, 0.05, 0.004), (x, -0.06, zz), "Steel", rot=(-45, 0, -30))
    rags = [(r.uniform(-1.8, 1.8), r.choice(strands[1:]), r.choice(["Fabric red", "Crew orange", "Fabric ochre"])) for _ in range(3)]
    for x, z, mat in rags:
        p.box((0.05, 0.01, 0.28), (x, -0.07, z - 0.15), mat, rot=(0, r.uniform(-15, 15), 0))
    with p.lowpoly():
        p.span((-2.05, -0.05, 0), (-1.95, 0.05, 1.35), "Rust")
        p.span((-0.05, -0.05, 0), (0.05, 0.05, 1.3), "Wood")
        for z, sag in zip(strands, sags):
            for x0 in (-2.0, 0.0):
                # Each span as two straight lengths dipping to its sag.
                mid = (x0 + 1.0, z - sag)
                for (xa, za), (xb, zb) in (((x0, z), mid), (mid, (x0 + 2.0, z))):
                    p.prism([(xa, za - 0.006), (xb, zb - 0.006), (xb, zb + 0.006), (xa, za + 0.006)], 0.01, (0, -0.06, 0), "Steel")
        for x, z, mat in rags:
            p.span((x - 0.025, -0.08, z - 0.29), (x + 0.025, -0.06, z - 0.01), mat)
    p.collider((4.0, 0.2, 1.2), (0, 0, 0.6))
    return p


def signpost(style: Style) -> Piece:
    """A Fringe signpost: a rusted post wedged in a cairn, arrow boards of
    plank nailed to it and stencilled with distances, and a yellow plate
    with a mask on it, because past here you need one."""
    p = Piece("signpost", "prop", "2.75 m signpost on a cairn; no collider")
    r = rng(style, p.name)
    p.cyl(0.05, 2.7, (0, 0, 1.35), "Rust", segments=10)
    p.cyl(0.06, 0.04, (0, 0, 2.7), "Hull dark", segments=10)
    # The cairn round its foot.
    for i in range(7):
        a = math.tau * i / 7
        p.sphere(0.15, (math.cos(a) * 0.22, math.sin(a) * 0.22, 0.08), ROCK, scale=(1.2, 1.0, 0.8),
                 rot=(0, 0, math.degrees(a)), segments=8, rings=5)
    for i in range(3):
        a = math.tau * i / 3 + 0.5
        p.sphere(0.12, (math.cos(a) * 0.12, math.sin(a) * 0.12, 0.25), ROCK, scale=(1.1, 1.0, 0.8), segments=8, rings=5)
    # Arrow boards: two planks each, nailed to the post with a clamp.
    arrow = [(0, -0.1), (0.78, -0.1), (0.78, -0.17), (1.0, 0), (0.78, 0.17), (0.78, 0.1), (0, 0.1)]
    for mat, z, turn, text in (("Sunbleached paint", 2.4, 0, "12"), ("Repaint teal", 2.05, 180, "40"), ("Crew orange", 1.72, 90, "3")):
        p.prism(arrow, 0.035, (0, 0, z), mat, rot=(90, 0, turn))
        a = math.radians(turn)
        along = Vector((math.cos(a), math.sin(a), 0))
        face = Vector((math.sin(a), -math.cos(a), 0))
        p.cyl(0.07, 0.06, (0, 0, z), "Hull dark", segments=10)
        for d in (0.3, 0.6):
            p.cyl(0.008, 0.01, Vector((0, 0, z)) + along * d + face * 0.022, "Steel", rot=(90, 0, turn), segments=6)
        if text:
            with p.painted():
                p.stencil(text, Vector((0, 0, z)) + along * 0.45 + face * 0.023, 0.12, "Hull dark",
                          facing={0: "-Y", 180: "+Y", 90: "+X"}[turn])
    # The mask warning, on a plate bolted to the post facing the track.
    plate = Vector((0, -0.07, 1.25))
    p.box((0.5, 0.02, 0.5), plate, "Hazard yellow")
    _bolts(p, [plate + Vector((x, -0.012, z)) for x in (-0.22, 0.22) for z in (-0.22, 0.22)], 0.012, 0.01, "Steel",
           rot=(90, 0, 0))
    with p.painted():
        # The rebreather: dark mask and cans painted on, its face picked out
        # in yellow on top of that, each layer the better part of a
        # centimetre proud of the last.
        f = plate + Vector((0, -0.017, 0))
        p.cyl(0.14, 0.004, f + Vector((0, 0, 0.03)), "Rubber", rot=(90, 0, 0), segments=14)
        for x in (-0.12, 0.12):
            p.cyl(0.055, 0.004, f + Vector((x, 0, -0.09)), "Rubber", rot=(90, 0, 0), segments=10)
        p.box((0.3, 0.004, 0.03), f + Vector((0, 0, -0.2)), "Rubber")
        p.cyl(0.08, 0.004, f + Vector((0, -0.009, 0.06)), "Hazard yellow", rot=(90, 0, 0), segments=12)
    # A rag tied on, faded.
    p.box((0.05, 0.01, 0.35), (0.06, 0.0, 2.55), "Fabric red", rot=(0, 12, 0))
    return p


def filter_cache(style: Style) -> Piece:
    """A Fringer's cache: a ribbed box buried in the dust, its lid thrown
    back on its hinges over rows of filter cartridges (the Fringe's money),
    a tarp half pulled off it, and a flag on a pole to find it by."""
    p = Piece("filter_cache", "prop", "1 × 0.7 m buried cache, flag to 2.4 m")
    r = rng(style, p.name)
    # The box: walls round a dark inside, ribbed, its rim just out of the dust.
    for lo, hi in (((-0.5, -0.35, -0.6), (0.5, -0.3, 0.15)), ((-0.5, 0.3, -0.6), (0.5, 0.35, 0.15)),
                   ((-0.5, -0.3, -0.6), (-0.45, 0.3, 0.15)), ((0.45, -0.3, -0.6), (0.5, 0.3, 0.15))):
        p.span(lo, hi, "Crew grey")
    for x in (-0.3, 0.0, 0.3):
        p.span((x - 0.025, -0.37, -0.1), (x + 0.025, -0.35, 0.15), "Crew grey")
    p.span((-0.45, -0.3, -0.12), (0.45, 0.3, -0.08), "Char")
    # The lid, thrown back on hinges, its handles.
    p.box((1.0, 0.7, 0.05), (0, 0.42, 0.5), "Repaint teal", rot=(-100, 0, 0))
    for x in (-0.3, 0.3):
        p.cyl(0.02, 0.1, (x, 0.36, 0.16), "Steel", rot=(0, 90, 0), segments=8)
        p.torus(0.05, 0.008, (x, 0.48, 0.55), "Hull dark", rot=(10, 0, 0), segments=8, sides=4, arc=180)
    # The cartridges, in rows, a gap where some have gone.
    for i in range(4):
        for j in range(3):
            if (i, j) == (3, 2):
                continue
            x, y = -0.32 + i * 0.21, -0.18 + j * 0.18
            p.cyl(0.068, 0.18, (x, y, 0.0), "Sunbleached paint", segments=8)
            p.cyl(0.078, 0.05, (x, y, 0.06), "Crew orange", segments=8)
            with p.painted():
                p.cyl(0.07, 0.02, (x, y, 0.1), "Hull dark", segments=8)
                p.cyl(0.025, 0.02, (x, y, 0.115), "Brass", segments=8)
    # A tarp half off it, dust drifted round.
    p.cloth([(-0.55, -0.9, 0.05), (0.0, -0.95, 0.05), (0.1, -0.35, 0.17), (-0.55, -0.35, 0.17)], "Canvas", sag=-0.05,
            ripple=0.02, cell=0.07, seed=5)
    for (x, y), rx, ry, h in (((-0.85, 0), 0.45, 0.7, 0.18), ((0.85, 0.1), 0.45, 0.65, 0.2), ((0, -0.7), 0.85, 0.38, 0.12)):
        _drift(p, (x, y, 0), rx, ry, h, "Dust red", r, rings=4, segments=14, lumps=1)
    # The flag on its pole.
    p.cyl(0.02, 2.4, (0.62, -0.42, 1.2), "Hull alloy", segments=8)
    p.sphere(0.03, (0.62, -0.42, 2.41), "Brass", segments=8, rings=4)
    p.cloth([(0.64, -0.42, 2.38), (1.1, -0.38, 2.36), (1.08, -0.4, 2.0), (0.64, -0.42, 2.02)], "Fabric red", sag=0.03,
            ripple=0.03, thickness=0.006, cell=0.05, droop=(0.3, 0, 0.3, 0), seed=9)
    p.collider((1.0, 0.7, 0.15), (0, 0, 0.075))
    return p


def windsock(style: Style) -> Piece:
    """A windsock on a guyed pole, its stripes bleached by the sun: how the
    Fringe watches for storms. The sock hangs half filled, drooping off
    downwind along +X."""
    p = Piece("windsock", "prop", "4.4 m windsock; no collider")
    r = rng(style, p.name)
    top = 4.0
    p.lathe([(0, 0), (0.3, 0), (0.3, 0.2), (0.22, 0.32), (0, 0.32)], (0, 0, 0), "Concrete", segments=10, smooth=False)
    p.cyl(0.05, top - 0.1, (0, 0, (top + 0.1) / 2), "Sunbleached paint", segments=10)
    p.cyl(0.07, 0.16, (0, 0, top), "Hull dark", segments=10)
    p.sphere(0.05, (0, 0, top + 0.35), "Medical red", segments=8, rings=6)
    p.cyl(0.015, 0.3, (0, 0, top + 0.18), "Steel", segments=6)
    for i in range(3):
        a = math.tau * i / 3 + 0.5
        foot = Vector((math.cos(a) * 1.6, math.sin(a) * 1.6, 0.02))
        p.cable((0, 0, top * 0.7), foot, 0.005, "Steel", sag=0.04, steps=6, segments=4)
        p.prism([(-0.03, 0), (0.03, 0), (0.04, 0.25), (-0.04, 0.25)], 0.05, foot - Vector((0, 0, 0.18)), "Rust")
    # The mouth: a hoop on a swivel arm.
    mouth = Vector((0.16, 0, top))
    p.tube([(0, 0, top), mouth], 0.02, "Hull dark", segments=6)
    p.torus(0.3, 0.02, mouth, "Hull dark", rot=(0, 90, 0), segments=16, sides=6)
    # The sock: rings sagging along a curve, five bands of colour.
    bands = ["Crew orange", "Canvas", "Crew orange", "Canvas", "Crew orange"]
    n = len(bands) * 2
    centres, radii = [], []
    for i in range(n + 1):
        t = i / n
        centres.append(mouth + Vector((1.5 * t, 0.06 * math.sin(t * 3), -0.75 * t * t)))
        radii.append(0.29 * (1 - 0.55 * t))
    for b, mat in enumerate(bands):
        rings = []
        for i in (2 * b, 2 * b + 1, 2 * b + 2):
            c, rr = centres[i], radii[i]
            nxt = centres[min(i + 1, n)] - centres[max(i - 1, 0)]
            ax = nxt.normalized()
            u = ax.cross(Vector((0, 0, 1))).normalized()
            v = u.cross(ax).normalized()
            ring = []
            for k in range(14):
                a = math.tau * k / 14
                squash = 1 - 0.25 * (i / n) * (1 + math.sin(a)) / 2
                ring.append(c + (u * math.cos(a) + v * math.sin(a) * squash) * rr)
            rings.append(ring)
        _loft(p, rings, mat, caps=(b == len(bands) - 1))
    return p


# Plants -------------------------------------------------------------------------

def quiver_tree(style: Style) -> Piece:
    """A quiver tree: a fat, pale, forked trunk under rosettes of thick
    leaves, the kind of hardy succulent that took on the Red where the
    terraformers got the air half right."""
    p = Sourced("quiver_tree", "prop", "about 1.4 × 1.2 m, 3.2 m tall", asset="quiver_tree_01", res="1k", height=3.2,
                budget=8000)
    p.collider((0.35, 0.35, 2.2), (0, 0, 1.1))
    return p


def dead_tree(style: Style) -> Piece:
    """A dead tree, fallen and bleached: a trunk from the Company Years'
    plantings that didn't take, lying in the dust where the storms left it.
    Low enough to vault along most of its length."""
    p = Sourced("dead_tree", "prop", "4.2 m fallen trunk", asset="dead_tree_trunk_02", res="1k", length=4.2, budget=6000)
    p.collider((3.4, 0.6, 0.6), (0, 0, 0.3))
    return p


def dry_brush(style: Style) -> Piece:
    """A few tussocks of dry, hardy grass and twigs, bleached by the sun
    and reddened by the dust: what grows out on the Fringe without anyone's
    help. Lean blades left plain (see _plant). Walked through."""
    p = Piece("dry_brush", "prop", "about 2.1 × 0.8 m, 0.5 m high; no collider", budget=3000)
    r = rng(style, p.name)
    with p.plain():
        for cx, cy, size in ((-0.65, 0.05, 1.0), (0.2, -0.1, 0.8), (0.8, 0.15, 0.6)):
            for _ in range(int(34 * size)):
                yaw = r.uniform(0, 360)
                lean = r.uniform(8, 48)
                tall = r.uniform(0.25, 0.5) * (0.6 + 0.4 * size)
                at = (cx + r.uniform(-0.12, 0.12) * size, cy + r.uniform(-0.1, 0.1) * size, 0)
                # Wide enough at the foot that its colour reads past the
                # outline the game draws round it.
                p.prism([(-0.016, 0), (0.016, 0), (0.004, tall * 0.7), (0.0, tall)], 0.002, at,
                        r.choice(["Wheat", "Bleached", "Bleached", "Fabric sand", "Dust red"]), rot=(lean, 0, yaw))
            for _ in range(3):
                a = math.radians(r.uniform(0, 360))
                tip = (cx + math.cos(a) * 0.3 * size, cy + math.sin(a) * 0.2 * size, r.uniform(0.25, 0.45) * size)
                p.tube([(cx, cy, 0), ((cx + tip[0]) / 2, (cy + tip[1]) / 2, tip[2] * 0.6), tip], 0.004, "Bleached", segments=3)
    return p


PIECES = [rock_small, rock_large, rock_spire, dust_mound, wind_turbine, greenhouse, crop_bed, irrigation_pipe,
          wrecked_terraformer, terraformer_debris, scrap_pile, salvage_frame, caravan_cart, fringer_tent, camp_stove,
          water_tank_fringe, wire_fence, signpost, filter_cache, windsock, quiver_tree, dead_tree, dry_brush]
