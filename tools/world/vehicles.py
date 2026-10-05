"""Vehicles: what's earned and what arrives. Players start on foot; haulers
and bikes are bought or given as gifts (docs/shared-world.md). Drifters
are the independent freighters that are the only link to Earth, and every
player steps off one, the Patience, at the Pads (docs/story.md). In Act 3
the Corvane Receivership comes down in a lander that's clean, new and
pressed, and looks wrong here on purpose (docs/look-and-feel.md).

Everything points its nose along -Y, the front, like every other piece
(the game's +Z, the way the characters face), with its wheels or feet on
z = 0. The ground vehicles are made along +X, which is easier to think
in, and turned at the end.

Parts of different colours never share a face plane (see GUIDE.md): what's
laid on a surface stands at least 6 mm off it, and anything thinner is
paint."""
import math

from kit import PALETTE, REPAINTS, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "Vehicles"

PALETTE.setdefault("Tyre", (0.025, 0.022, 0.02))
PALETTE.setdefault("Hull pale", (0.42, 0.43, 0.42))
PALETTE.setdefault("Polymer", (0.06, 0.06, 0.05))
# Tarps and nets big enough to cover a load: canvas, but not in finish.py's
# cloth (which roughens cloth finely all over), so they keep to their budget;
# kit's cloth gives them their folds.
PALETTE.setdefault("Tarp", PALETTE["Canvas"])


def _hanging(p: Piece, a, b, top: float, bottom: float, mat: str, ripple: float = 0.02, thickness: float = 0.012,
             cell: float = 0.3, sag: float = 0.0, seed: int = 0) -> None:
    """A sheet of cloth hanging straight down from a to b ((x, y) points)
    between heights top and bottom: a tarp's side, a net on a wall. kit's
    cloth gives a sheet its thickness downwards, which is right for one
    lying flat but leaves one hanging with no thickness at all (and an
    outline that has nothing to push out along); this gives it its
    thickness through the sheet. sag drops its middle along the top."""
    import bmesh  # noqa: F401
    from mathutils import Vector, noise
    a, b = Vector((*a, 0)), Vector((*b, 0))
    along = b - a
    n = Vector((-along.y, along.x, 0)).normalized()
    nu = max(2, round(along.length / cell))
    nv = max(2, round((top - bottom) / cell))
    front, back = [], []
    for j in range(nv + 1):
        v = j / nv
        for i in range(nu + 1):
            u = i / nu
            pt = a + along * u
            z = bottom + (top - bottom) * v - sag * 4 * u * (1 - u) * v
            off = ripple * noise.noise(Vector((pt.x * 2.3, pt.y * 2.3, z * 2.3 + seed * 5.1)))
            q = Vector((pt.x, pt.y, z)) + n * off
            front.append(p.bm.verts.new(q + n * thickness / 2))
            back.append(p.bm.verts.new(q - n * thickness / 2))
    w = nu + 1
    faces = []
    for j in range(nv):
        for i in range(nu):
            k = (j * w + i, j * w + i + 1, (j + 1) * w + i + 1, (j + 1) * w + i)
            faces.append(p.bm.faces.new([front[x] for x in k]))
            faces.append(p.bm.faces.new([back[x] for x in reversed(k)]))
    ring = list(range(nu)) + [nu + j * w for j in range(nv)] + [nv * w + nu - i for i in range(nu)] + [(nv - j) * w for j in range(nv)]
    for i0, i1 in zip(ring, ring[1:] + ring[:1]):
        faces.append(p.bm.faces.new((front[i1], front[i0], back[i0], back[i1])))
    p._faces(faces, mat, smooth=True)


# Wheels and running gear ---------------------------------------------------------
#
# The colony makes nothing new, so every vehicle here is hand-built from
# what the Second Light and the terraformers left: fat balloon tyres for the
# dust (the air's too thin to hover in), suspension out in the open where it
# can be mended, Crew-orange and Hazard-yellow accents on mismatched repaint.
# They take their lines from the sci-fi references in the brief (a
# cyberpunk bike's single-sided swingarm and long cowl, a six-wheel rover's
# balloon tyres and boxy modules, an explorer truck's capsule nose, a
# buggy's faceted tub and tube cage) and make them the Red's.

ACCENT = "Hazard yellow"


def _balloon(p: Piece, x, y, z, radius, width, side: int, blocks: int = 24, rim: str = "Hull alloy",
             cap: str = ACCENT, both: bool = False) -> None:
    """A fat balloon tyre on an axle along Y, its outer face towards side
    (+1 or -1 in Y); both puts a rim on each face (a bike's wheel). Bulged
    walls, a tread of chunky staggered blocks, a dished rim with a painted
    hub cap and its bolts."""
    r, w = radius, width
    rr = 0.56 * r
    rot = (-90, 0, 0) if side > 0 else (90, 0, 0)
    p.lathe([(rr, -w / 2 + 0.02), (0.72 * r, -w / 2), (0.88 * r, -0.47 * w), (0.97 * r, -0.38 * w), (r, -0.22 * w),
             (r, 0.22 * w), (0.97 * r, 0.38 * w), (0.88 * r, 0.47 * w), (0.72 * r, w / 2), (rr, w / 2 - 0.02)],
            (x, y, z), "Tyre", rot=rot, segments=10)
    # The tread: chunky blocks in two staggered rows, left plain (a bevel
    # on every block would multiply a tyre's triangles several times).
    with p.plain():
        for i in range(blocks):
            a = math.tau * i / blocks
            for row in (-1, 1):
                b = a + (math.pi / blocks if row > 0 else 0)
                c = (x + math.cos(b) * (r + 0.004), y + row * 0.24 * w, z + math.sin(b) * (r + 0.004))
                p.box((0.7 * math.tau * r / blocks, 0.42 * w, 0.03 * r / 0.6), c, "Tyre", rot=(0, 90 - math.degrees(b), 0))
    for s in ((-1, 1) if both else (side,)):
        rot_s = (-90, 0, 0) if s > 0 else (90, 0, 0)
        face = y + s * (w / 2 - 0.03)
        # A dished rim set into the tyre, the hub cap proud of it, its bolts.
        p.lathe([(0.0, 0.0), (rr, 0.0), (rr, 0.025), (rr * 0.8, 0.05), (rr * 0.42, 0.06), (0.0, 0.06)],
                (x, face - s * 0.06, z), rim, rot=rot_s, segments=10)
        p.lathe([(0.0, 0.0), (rr * 0.34, 0.0), (rr * 0.32, 0.05), (rr * 0.2, 0.09), (0.0, 0.1)],
                (x, face - s * 0.005, z), cap, rot=rot_s, segments=8)
        with p.painted():
            for i in range(5):
                a = math.tau * i / 5
                p.cyl(0.016 * r / 0.5, 0.012, (x + math.cos(a) * rr * 0.62, face + s * 0.012, z + math.sin(a) * rr * 0.62),
                      "Steel", rot=(90, 0, 0), segments=5)


def _source(p: Piece, asset: str, at, rot=(0, 0, 0), **fit) -> None:
    """p.source for a piece made along +X and turned to -Y at the end: kit's
    turn() turns the piece's own geometry and colliders but not its sourced
    parts, so these are placed where they'll be after the quarter turn."""
    x, y, z = at
    p.source(asset, at=(y, -x, z), rot=(rot[0], rot[1], rot[2] - 90), **fit)


def _coilover(p: Piece, a, b, radius: float = 0.06, turns: int = 7, spring: str = ACCENT) -> None:
    """A coilover from a to b: a damper body and rod, the spring wound round
    them (left plain: a wire, every turn of it)."""
    from mathutils import Vector
    a, b = Vector(a), Vector(b)
    axis = (b - a)
    length = axis.length
    d = axis.normalized()
    up = Vector((0, 0, 1)) if abs(d.z) < 0.9 else Vector((1, 0, 0))
    u = d.cross(up).normalized()
    v = d.cross(u)
    mid = a + axis * 0.55
    p.tube([a, mid], radius * 0.55, "Gunmetal", segments=5)
    p.tube([mid - d * 0.02, b], radius * 0.25, "Steel", segments=4)
    pts = []
    steps = turns * 8
    for i in range(steps + 1):
        t = i / steps
        ang = math.tau * turns * t
        pts.append(a + axis * (0.1 + 0.8 * t) + (u * math.cos(ang) + v * math.sin(ang)) * radius)
    with p.plain():
        p.tube(pts, radius * 0.18, spring, segments=3)
    for e in (a, b):
        p.sphere(radius * 0.5, e, "Hull dark", segments=6, rings=4)


def _arm(p: Piece, a, b, radius: float = 0.05, mat: str = "Hull dark") -> None:
    p.tube([a, b], radius, mat, segments=4)


def _light_bar(p: Piece, x0, x1, y, z, lamps: int = 4, along: str = "y") -> None:
    """A light bar on a bracket: a dark housing and its lamps, facing +X."""
    if along == "y":
        p.span((x0 - 0.06, y - (x1 - x0) / 2, z - 0.07), (x0 + 0.06, y + (x1 - x0) / 2, z + 0.07), "Hull dark")
        for i in range(lamps):
            yy = y - (x1 - x0) / 2 + (x1 - x0) * (i + 0.5) / lamps
            p.cyl(0.05, 0.03, (x0 + 0.07, yy, z), "Sodium lamp", rot=(0, 90, 0), segments=6)


# Hauler: a six-wheel rover-truck, made with its cab at +X and its bed behind --

def _hauler_chassis(p: Piece, style: Style) -> tuple[str, str]:
    """The rover-truck's running gear and its capsule-nosed cab: a spine
    and side modules over six balloon tyres on trailing arms and coilovers
    (the yellow six-wheel rover's stance, the explorer truck's nose).
    Returns its two paints."""
    r = rng(style, p.name)
    paint = r.choice(["Crew orange", "Hazard yellow", "Repaint cream", "Crew orange"])
    patch = r.choice([c for c in REPAINTS if c != paint])
    w = 1.25
    # The spine, its cross members, and the side modules between the wheels.
    p.span((-4.1, -0.6, 0.75), (4.0, 0.6, 1.15), "Hull dark")
    for x in (-3.6, -1.9, 0.2, 2.0, 3.6):
        p.span((x - 0.07, -1.0, 0.82), (x + 0.07, 1.0, 1.08), "Hull dark")
    for s in (-1, 1):
        y0, y1 = sorted((s * 0.62, s * (w - 0.03)))
        # The battery and air module between the middle and front wheels,
        # clear of both arches; its door panels a little proud of it.
        p.span((0.0, y0, 0.72), (2.05, y1, 1.36), patch)
        for x in (0.4, 1.05, 1.7):
            p.span((x - 0.25, min(s * (w - 0.015), s * (w + 0.015)), 0.86), (x + 0.25, max(s * (w - 0.015), s * (w + 0.015)), 1.22), paint)
        with p.painted():
            for i in range(5):
                p.box((0.1, 0.012, 0.22), (0.25 + i * 0.36, s * (w + 0.03), 1.04), "Hazard yellow",
                      rot=(0, 35, 0))
        # The steps up to the cab door, in front of the module.
        b0, b1 = sorted((s * 0.9, s * 1.32))
        for z in (0.55, 0.85):
            p.span((2.29, b0, z), (2.58, b1, z + 0.04), "Grating")
        for x in (2.25, 2.58):
            p.span((x, b0, 0.5), (x + 0.04, b1, 1.15), "Hull dark")
    # Six balloon tyres on trailing arms, each with its coilover.
    for x in (3.05, -1.0, -2.85):
        for s in (-1, 1):
            y = s * 1.32
            _balloon(p, x, y, 0.65, 0.65, 0.52, s, blocks=24)
            pivot = (x + 0.75 if x > 0 else x + 0.7, s * 0.75, 0.95)
            _arm(p, pivot, (x, s * 0.98, 0.65), 0.07, "Gunmetal")
            _coilover(p, (x + 0.25, s * 0.95, 0.72), (x + 0.45, s * 0.72, 1.45), radius=0.07, turns=6)
    # Mudguards: faceted arches over each wheel.
    for x in (3.05, -1.0, -2.85):
        for s in (-1, 1):
            y0, y1 = sorted((s * 1.02, s * 1.62))
            p.prism([(x - 0.85, 1.12), (x - 0.55, 1.5), (x + 0.55, 1.5), (x + 0.85, 1.12), (x + 0.8, 1.06), (x + 0.5, 1.42),
                      (x - 0.5, 1.42), (x - 0.8, 1.06)], abs(y1 - y0), (0, (y0 + y1) / 2, 0), "Hull dark")
    # The cab: a capsule nose in profile, extruded across; glass in its own
    # plane, a band round it, the twin lamps, a bumper and tow hooks.
    nose = [(1.7, 1.15), (3.95, 1.15), (4.3, 1.45), (4.36, 1.95), (4.06, 2.6), (3.45, 2.98), (1.7, 2.98)]
    p.prism(nose, 2 * w, (0, 0, 0), paint)
    slope = math.degrees(math.atan2(0.3, 0.6))
    p.box((0.02, 2 * w - 0.3, 0.64), (4.23, 0, 2.29), "Glass", rot=(0, -slope, 0))
    for s in (-1, 1):
        p.prism([(2.3, 2.05), (3.95, 2.05), (4.02, 2.52), (3.4, 2.84), (2.3, 2.84)], 0.012, (0, s * (w + 0.008), 0), "Glass")
        # The door: a seam and a handle (paint), hinges proud of the body.
        with p.painted():
            p.span((2.2, s * (w + 0.007) - 0.002, 1.25), (2.24, s * (w + 0.007) + 0.002, 2.0), "Hull dark")
            p.span((3.3, s * (w + 0.007) - 0.002, 1.25), (3.34, s * (w + 0.007) + 0.002, 2.0), "Hull dark")
            p.span((1.72, s * (w + 0.007) - 0.002, 1.92), (4.0, s * (w + 0.007) + 0.002, 2.0), "Hull dark")
            p.stencil("27", (2.75, s * (w + 0.008), 1.6), 0.28, "Stencil white", facing="-Y" if s < 0 else "+Y")
        for z in (1.35, 1.85):
            p.cyl(0.025, 0.12, (2.22, s * (w + 0.02), z), "Steel", segments=5)
        # Twin lamps either side of the nose, and the mirrors.
        for dz in (0.0, 0.2):
            p.lathe([(0.0, 0.0), (0.1, 0.0), (0.11, 0.04), (0.0, 0.06)], (4.31, s * 0.82, 1.55 + dz), "Hull dark", rot=(0, 90, 0), segments=6)
            p.cyl(0.08, 0.02, (4.37, s * 0.82, 1.55 + dz), "Sodium lamp", rot=(0, 90, 0), segments=6)
        p.tube([(3.9, s * w, 2.4), (3.95, s * (w + 0.3), 2.5), (3.95, s * (w + 0.3), 2.75)], 0.02, "Hull dark", segments=4)
        p.span((3.9, s * (w + 0.26), 2.55), (4.0, s * (w + 0.42), 2.85), "Hull dark")
    with p.painted():
        for i in range(5):
            p.box((0.012, 0.16, 0.24), (4.335, -0.48 + i * 0.24, 1.25), "Hazard yellow", rot=(35, 0, 0))
    p.span((4.0, -w - 0.05, 0.85), (4.45, w + 0.05, 1.12), "Gunmetal")
    for y in (-0.7, 0.7):
        p.torus(0.08, 0.022, (4.5, y, 0.95), "Steel", rot=(90, 0, 0), segments=6, sides=4)
    # The roof: a light bar, a rack, a beacon and two aerials.
    _light_bar(p, 3.55, 5.15, 0, 3.08, lamps=5)
    for y in (-0.95, 0.95):
        p.tube([(1.85, y, 3.12), (3.3, y, 3.12)], 0.025, "Hull dark", segments=4)
    for x in (2.1, 2.6, 3.1):
        p.tube([(x, -0.95, 3.12), (x, 0.95, 3.12)], 0.02, "Hull dark", segments=4)
    p.lathe([(0.0, 0.0), (0.08, 0.0), (0.07, 0.1), (0.0, 0.12)], (2.0, 0.6, 2.98), "Hazard yellow", segments=6)
    with p.plain():
        for y in (-1.05, 1.05):
            p.cyl(0.008, 1.3, (1.85, y, 3.6), "Hull dark", segments=3)
    # Seats and the wheel, seen through the glass.
    for y in (-0.55, 0.55):
        p.span((2.4, y - 0.28, 1.5), (2.9, y + 0.28, 1.65), "Leather")
        p.span((2.35, y - 0.28, 1.65), (2.47, y + 0.28, 2.3), "Leather")
    p.torus(0.2, 0.02, (3.4, 0.55, 1.95), "Polymer", rot=(0, 60, 0), segments=8, sides=4)
    p.collider((2.7, 2 * w, 1.85), (3.03, 0, 1.15 + 1.85 / 2))
    p.collider((8.2, 2 * w, 0.62), (-0.05, 0, 0.72 + 0.31))
    return paint, patch


def hauler(style: Style) -> Piece:
    """The Pads-to-Hull haul contracts' workhorse: a six-wheel rover-truck
    on balloon tyres, a capsule cab up front and a planked bed behind, its
    load of crates lashed down under a tarp. Bought, or given as a faction's
    favour."""
    p = Piece("hauler", "vehicle", "8.6 × 3.2 × 3.6 m rover-truck, cab at -Y")
    p.budget = 60000
    paint, patch = _hauler_chassis(p, style)
    r = rng(style, p.name + "bed")
    w = 1.25
    # The bed: a steel deck under planks, side rails with stake pockets.
    p.span((-4.15, -w, 1.4), (1.6, w, 1.55), "Hull alloy")
    for i in range(10):
        y0 = -w + 0.02 + i * 0.248
        p.span((-4.13, y0, 1.55), (1.58, y0 + 0.23, 1.57), r.choice(["Wood", "Wood", "Wood dark"]))
    for s in (-1, 1):
        y0, y1 = sorted((s * (w - 0.04), s * (w + 0.04)))
        p.span((-4.17, y0, 1.47), (1.62, y1, 1.67), paint)
        for x in (-3.7, -2.5, -1.3, -0.1, 1.1):
            p.span((x - 0.05, y0 - 0.01, 1.67), (x + 0.05, y1 + 0.01, 1.8), "Hull dark")
    p.tube([(1.55, -w + 0.05, 1.57), (1.55, -w + 0.05, 3.0), (1.55, w - 0.05, 3.0), (1.55, w - 0.05, 1.57)], 0.04, "Hull dark", segments=4)
    p.span((1.53, -w + 0.1, 1.61), (1.57, w - 0.1, 2.95), "Grating")
    # The load: crates two high, lashed, the front of it under a tarp.
    stacks = [(-3.5, 1.0), (-2.25, 1.15), (-1.0, 1.15), (0.3, 1.1)]
    top = 0
    for x, h in stacks:
        for y in (-0.6, 0.6):
            hh = h * r.uniform(0.85, 1.0)
            mat = r.choice(["Crew orange", "Container blue", "Container green", "Repaint cream"])
            p.span((x - 0.58, y - 0.55, 1.57), (x + 0.58, y + 0.55, 1.57 + hh), mat)
            p.span((x - 0.6, y - 0.57, 1.57 + hh - 0.08), (x + 0.6, y + 0.57, 1.57 + hh + 0.012), "Hull dark")
            top = max(top, hh)
    lid = 1.57 + top + 0.05
    p.cloth([(-2.9, -w - 0.06, lid), (1.0, -w - 0.06, lid), (1.0, w + 0.06, lid), (-2.9, w + 0.06, lid)],
            "Tarp", sag=-0.04, ripple=0.03, thickness=0.012, cell=0.3, seed=3, droop=(0.2, 0, 0.2, 0))
    for s in (-1, 1):
        _hanging(p, (-2.9, s * (w + 0.065)), (1.0, s * (w + 0.065)), lid, 1.85, "Tarp", ripple=0.025, thickness=0.014,
                 cell=0.3, seed=5 + s)
    for x in (-2.4, -1.0, 0.5, -3.5):
        p.tube([(x, -w - 0.09, 1.65), (x, -w - 0.09, lid + 0.04), (x, w + 0.09, lid + 0.04), (x, w + 0.09, 1.65)], 0.016,
               "Hazard yellow", segments=4)
        p.span((x - 0.05, -w - 0.13, 1.75), (x + 0.05, -w - 0.09, 1.87), "Steel")
    p.collider((5.0, 2 * w - 0.2, top), (-1.6, 0, 1.57 + top / 2))
    p.turn(-1)  # nose along -Y
    return p


def hauler_tanker(style: Style) -> Piece:
    """The rover-truck with a tank on its back: water out to the Fringe,
    fuel to the Pads. A walkway and rails along the top, manholes, a ladder,
    a hose reel and valves at the back. Whoever drives it is carrying the
    colony's leverage."""
    p = Piece("hauler_tanker", "vehicle", "8.6 × 3.2 × 3.7 m rover-tanker, cab at -Y")
    p.budget = 60000
    paint, patch = _hauler_chassis(p, style)
    # The tank on its cradles, banded, domed at its ends.
    p.span((-4.1, -1.0, 1.15), (1.6, 1.0, 1.3), "Hull dark")
    p.lathe([(0.0, -2.88), (0.6, -2.85), (0.92, -2.75), (1.05, -2.6), (1.05, 2.6), (0.92, 2.75), (0.6, 2.85), (0.0, 2.88)],
            (-1.25, 0, 2.4), "Water blue", rot=(0, 90, 0), segments=11)
    for x in (-3.5, -2.0, -0.4, 1.0):
        p.torus(1.058, 0.04, (x, 0, 2.4), "Hull dark", rot=(0, 90, 0), segments=11, sides=4)
        p.span((x - 0.1, -0.85, 1.3), (x + 0.1, 0.85, 1.65), "Hull dark")
    p.span((-3.7, -0.3, 3.44), (1.2, 0.3, 3.5), "Grating")
    for s in (-1, 1):
        for x in (-3.5, -2.1, -0.7, 0.7):
            p.cyl(0.018, 0.9, (x, s * 0.36, 3.9), "Steel", segments=4)
        p.tube([(-3.5, s * 0.36, 4.35), (0.7, s * 0.36, 4.35)], 0.02, "Steel", segments=4)
    for x in (-2.7, -0.3):
        p.lathe([(0.0, 0.0), (0.26, 0.0), (0.26, 0.1), (0.22, 0.14), (0.0, 0.15)], (x, 0, 3.4), "Hull alloy", segments=8)
        p.torus(0.2, 0.02, (x, 0, 3.57), "Steel", segments=6, sides=4)
    for y in (0.5, 0.9):
        p.tube([(-4.18, y, 1.3), (-4.18, y, 3.5)], 0.025, "Steel", segments=4)
    for z in (1.6, 1.95, 2.3, 2.65, 3.0, 3.35):
        p.tube([(-4.18, 0.5, z), (-4.18, 0.9, z)], 0.018, "Steel", segments=4)
    p.cyl(0.3, 0.32, (-3.85, -0.6, 1.65), "Crew orange", rot=(0, 90, 0), segments=8)
    p.cyl(0.24, 0.34, (-3.85, -0.6, 1.65), "Rubber", rot=(0, 90, 0), segments=8)
    p.cable((-4.02, -0.6, 1.45), (-4.15, -0.9, 0.3), 0.035, "Rubber", sag=0.1, segments=5)
    for y in (-0.1, 0.2):
        p.lathe([(0.0, 0.0), (0.05, 0.0), (0.05, 0.2), (0.08, 0.22), (0.08, 0.3), (0.0, 0.3)], (-4.12, y, 1.5), "Brass",
                rot=(0, -90, 0), segments=5)
    with p.painted():
        p.stencil("40", (-2.3, -1.068, 2.95), 0.32, "Stencil white")
        for x in (-3.1, 0.6):
            p.box((0.32, 0.01, 0.32), (x, -1.065, 2.25), "Hazard yellow", rot=(0, 45, 0))
    p.collider((5.8, 2.1, 2.3), (-1.25, 0, 1.15 + 2.3 / 2))
    p.turn(-1)  # nose along -Y
    return p


# Bike, trike and buggy --------------------------------------------------------

def _nose_lamps(p: Piece, x, z, gap: float, mat: str = "Laser cyan") -> None:
    """Twin round lamps set into a cowl's nose, facing +X."""
    for y in (-gap / 2, gap / 2):
        p.lathe([(0.0, 0.0), (0.05, 0.0), (0.055, 0.03), (0.0, 0.04)], (x, y, z), "Hull dark", rot=(0, 90, 0), segments=6)
        p.cyl(0.038, 0.02, (x + 0.04, y, z), mat, rot=(0, 90, 0), segments=6)


def bike(style: Style) -> Piece:
    """A frontier courier bike after the cyberpunk-bike concept: fat balloon
    tyres, a single-sided swingarm in Hazard yellow, a boxy electric drive
    out in the open between the wheels, and one long low cowl from the seat
    to past the front wheel, a ridge down its back, twin lamps in its nose.
    Patched, dusty, a courier's bag strapped on."""
    p = Piece("bike", "vehicle", "2.3 × 0.7 × 1.1 m bike, front at -Y")
    p.budget = 30000
    r = rng(style, p.name)
    paint = r.choice(["Hull alloy", "Repaint teal", "Repaint cream", "Crew grey"])
    ridge = r.choice([c for c in REPAINTS + ["Crew orange"] if c != paint])
    rad = 0.34
    _balloon(p, -0.8, 0, rad, rad, 0.24, -1, both=True)
    _balloon(p, 0.82, 0, rad, rad, 0.2, -1, both=True)
    # The swingarm: one deep yellow arm on the left, tapering from the pivot
    # to the rear hub, and its shock up to the frame.
    p.prism([(-0.86, 0.29), (-0.12, 0.36), (-0.1, 0.54), (-0.86, 0.41)], 0.07, (0, -0.16, 0), ACCENT)
    p.cyl(0.07, 0.1, (-0.12, -0.16, 0.45), "Gunmetal", rot=(90, 0, 0), segments=6)
    _coilover(p, (-0.4, -0.09, 0.5), (-0.22, -0.06, 0.84), radius=0.035, turns=6)
    # The fork: two straight raked legs, yellow sliders on steel tubes, a
    # fender over the tyre.
    for s in (-1, 1):
        p.tube([(0.82, s * 0.13, rad), (0.7, s * 0.13, 0.66)], 0.034, ACCENT, segments=4)
        p.tube([(0.71, s * 0.12, 0.62), (0.58, s * 0.1, 0.98)], 0.024, "Steel", segments=4)
    for z in (0.86, 0.97):
        p.span((0.53, -0.13, z - 0.02), (0.63, 0.13, z + 0.02), "Hull dark")
    arc = [(0.82 + math.cos(math.radians(a)) * (rad + 0.05), rad + math.sin(math.radians(a)) * (rad + 0.05)) for a in range(20, 150, 16)]
    p.prism(arc + [(x - (x - 0.82) * 0.12, z - 0.03) for x, z in reversed(arc)], 0.21, (0, 0, 0), "Hull dark")
    # The frame: a spine from the head back over the drive to the tail.
    p.tube([(0.58, 0, 0.96), (0.1, 0, 0.78), (-0.4, 0, 0.74), (-0.8, 0, 0.8)], 0.035, "Hull dark", segments=4)
    p.tube([(0.56, 0, 0.9), (0.42, 0, 0.42), (0.02, 0, 0.3), (-0.12, 0, 0.45)], 0.03, "Hull dark", segments=4)
    # The drive: a motor drum with yellow rims, a finned battery box, the
    # controller and its cables, all out in the open.
    p.cyl(0.16, 0.26, (-0.05, 0, 0.45), "Gunmetal", rot=(90, 0, 0), segments=10)
    for s in (-1, 1):
        p.torus(0.16, 0.02, (-0.05, s * 0.14, 0.45), ACCENT, rot=(90, 0, 0), segments=10, sides=4)
    p.span((0.1, -0.12, 0.4), (0.44, 0.12, 0.68), "Hull dark")
    for i in range(6):
        z = 0.42 + i * 0.042
        p.span((0.44, -0.11, z), (0.48, 0.11, z + 0.018), "Gunmetal")
    p.span((0.14, -0.13, 0.46), (0.4, -0.12, 0.62), ridge)
    with p.painted():
        p.stencil("9", (0.27, -0.136, 0.54), 0.1, "Stencil white")
    with p.plain():
        for z in (0.52, 0.6):
            p.cable((0.12, 0.1, z), (-0.1, 0.1, z - 0.15), 0.01, "Polymer", sag=0.03, steps=4, segments=3)
    # The exhaust: a chrome loop from the drive's cooler back under the tail.
    p.tube([(-0.05, 0.14, 0.6), (-0.3, 0.17, 0.66), (-0.6, 0.17, 0.6), (-0.9, 0.15, 0.66)], 0.028, "Steel", segments=5)
    # The cowl: low and long, seat to nose, its flank panel and a ridge down
    # its back in another paint; the twin lamps in its nose.
    p.prism([(-0.35, 0.76), (0.5, 0.78), (0.94, 0.84), (1.0, 0.94), (0.72, 1.01), (0.1, 0.99), (-0.3, 0.93)], 0.32, (0, 0, 0), paint)
    p.prism([(-0.2, 0.92), (0.62, 0.99), (0.66, 1.05), (0.1, 1.07), (-0.18, 0.98)], 0.16, (0, 0, 0), ridge)
    with p.painted():
        for s in (-1, 1):
            p.prism([(0.15, 0.82), (0.6, 0.84), (0.55, 0.92), (0.2, 0.9)], 0.006, (0, s * 0.165, 0), "Hull dark")
    _nose_lamps(p, 0.99, 0.9, 0.12)
    # The seat, low and long, the upswept tail, the tail lamp.
    p.prism([(-0.85, 0.82), (-0.3, 0.84), (-0.25, 0.92), (-0.8, 0.92)], 0.24, (0, 0, 0), "Leather")
    p.prism([(-1.08, 0.82), (-0.82, 0.8), (-0.78, 0.93), (-1.02, 0.98)], 0.2, (0, 0, 0), paint)
    p.span((-1.1, -0.07, 0.86), (-1.06, 0.07, 0.92), "Fabric red")
    # Bars, grips.
    p.tube([(0.5, -0.33, 1.06), (0.58, -0.22, 1.0), (0.58, 0.22, 1.0), (0.5, 0.33, 1.06)], 0.014, "Hull dark", segments=4)
    for s in (-1, 1):
        p.cyl(0.022, 0.11, (0.5, s * 0.33, 1.06), "Rubber", rot=(90, 0, 0), segments=5)
    # A courier bag strapped on the right of the tail.
    p.span((-0.9, 0.13, 0.62), (-0.6, 0.26, 0.8), "Canvas")
    p.span((-0.91, 0.124, 0.74), (-0.59, 0.266, 0.82), "Fabric olive")
    p.collider((2.2, 0.5, 1.0), (0, 0, 0.6))
    p.turn(-1)  # nose along -Y
    return p


def trike(style: Style) -> Piece:
    """A three-wheeled Fringe runabout in the bike's family: one fat tyre
    up front on a yellow fork under a faceted cowl with twin lamps, two
    balloon tyres behind under a tub, a bench under a roll bar, and a rack of
    water cans and a crate roped down behind."""
    p = Piece("trike", "vehicle", "2.8 × 1.8 × 1.75 m trike, front at -Y")
    p.budget = 30000
    r = rng(style, p.name)
    paint = r.choice(["Bleached", "Repaint green", "Repaint cream", "Crew grey"])
    _balloon(p, 1.0, 0, 0.38, 0.38, 0.24, -1, blocks=24, both=True)
    for s in (-1, 1):
        _balloon(p, -0.85, s * 0.66, 0.44, 0.44, 0.32, s, blocks=24)
    p.cyl(0.05, 1.3, (-0.85, 0, 0.44), "Hull dark", rot=(90, 0, 0), segments=5)
    p.lathe([(0.0, -0.14), (0.12, -0.13), (0.16, 0.0), (0.12, 0.13), (0.0, 0.14)], (-0.85, 0, 0.44), "Gunmetal", rot=(90, 0, 0), segments=6)
    for s in (-1, 1):
        _coilover(p, (-0.7, s * 0.4, 0.5), (-0.45, s * 0.38, 0.95), radius=0.045, turns=6)
        p.tube([(1.0, s * 0.15, 0.38), (0.82, s * 0.13, 0.7), (0.7, s * 0.1, 1.02)], 0.032, ACCENT, segments=4)
    # The tub over the back axle and the frame forward to the head.
    p.prism([(-1.35, 0.5), (0.25, 0.5), (0.35, 0.8), (0.2, 0.98), (-1.3, 0.98), (-1.42, 0.75)], 1.0, (0, 0, 0), paint)
    p.tube([(-0.2, 0, 0.55), (0.45, 0, 0.58), (0.72, 0, 0.98)], 0.04, "Hull dark", segments=4)
    # The cowl over the fork, its lamps; the bars.
    p.prism([(0.3, 0.82), (0.92, 0.9), (1.02, 1.0), (0.75, 1.1), (0.35, 1.05)], 0.36, (0, 0, 0), paint)
    _nose_lamps(p, 1.0, 0.96, 0.16, "Sodium lamp")
    p.tube([(0.62, -0.42, 1.22), (0.72, -0.3, 1.14), (0.72, 0.3, 1.14), (0.62, 0.42, 1.22)], 0.018, "Hull dark", segments=4)
    for s in (-1, 1):
        p.cyl(0.024, 0.12, (0.62, s * 0.42, 1.22), "Rubber", rot=(90, 0, 0), segments=5)
        # Fenders over the rear tyres.
        y0, y1 = sorted((s * 0.45, s * 0.88))
        p.prism([(-1.4, 0.7), (-1.2, 0.95), (-0.5, 0.95), (-0.3, 0.7), (-0.34, 0.66), (-0.52, 0.9), (-1.18, 0.9), (-1.36, 0.66)],
                abs(y1 - y0), (0, (y0 + y1) / 2, 0), "Hull dark")
    # The bench and the roll bar.
    p.span((-0.3, -0.36, 0.98), (0.1, 0.36, 1.08), "Leather")
    p.span((-0.4, -0.36, 1.08), (-0.3, 0.36, 1.4), "Leather")
    p.tube([(-0.47, -0.5, 0.98), (-0.47, -0.5, 1.72), (-0.47, 0.5, 1.72), (-0.47, 0.5, 0.98)], 0.035, "Hull dark", segments=4)
    for s in (-1, 1):
        p.tube([(-0.47, s * 0.5, 1.6), (-1.28, s * 0.48, 0.99)], 0.025, "Hull dark", segments=4)
    _light_bar(p, -0.42, 0.42, 0, 1.76, lamps=3)
    # The rack: water cans and a crate, roped down.
    p.span((-1.32, -0.5, 0.98), (-0.52, 0.5, 1.02), "Grating")
    for y in (-0.3, 0.05):
        _source(p, "metal_jerrycan_green", (-1.12, y, 1.02), (0, 0, 90), height=0.46)
    _source(p, "plastic_crate_02", (-0.72, 0.0, 1.02), (0, 0, 90), length=0.4)
    p.cable((-1.3, -0.52, 1.47), (-0.52, -0.52, 1.32), 0.01, "Canvas", sag=0.03, segments=4)
    p.cable((-1.3, 0.52, 1.47), (-0.52, 0.52, 1.32), 0.01, "Canvas", sag=0.03, segments=4)
    p.collider((2.4, 1.2, 1.1), (-0.15, 0, 0.6 + 0.4))
    p.turn(-1)  # nose along -Y
    return p


def buggy(style: Style) -> Piece:
    """A two-seat Fringe runabout after the sci-fi buggy: a faceted tub
    low between four balloon tyres on A-arms and yellow coilovers, a tube
    cage over the seats with a light bar, water cans and a spare on the back.
    What a Fringer who's done well drives."""
    p = Piece("buggy", "vehicle", "3.7 × 2.1 × 1.8 m buggy, front at -Y")
    p.budget = 40000
    r = rng(style, p.name)
    paint = r.choice(["Bleached", "Hull alloy", "Repaint oxide", "Crew orange"])
    for x in (1.3, -1.25):
        for s in (-1, 1):
            _balloon(p, x, s * 0.88, 0.44, 0.44, 0.32, s, blocks=24)
            # Upper and lower A-arms to the hub, and the coilover.
            for z0, z1 in ((0.38, 0.4), (0.72, 0.56)):
                _arm(p, (x - 0.22, s * 0.42, z0), (x, s * 0.72, z1), 0.025, "Gunmetal")
                _arm(p, (x + 0.22, s * 0.42, z0), (x, s * 0.72, z1), 0.025, "Gunmetal")
            _coilover(p, (x + (0.08 if x > 0 else -0.08), s * 0.68, 0.5), (x + (0.18 if x > 0 else -0.18), s * 0.45, 1.0),
                      radius=0.055, turns=7)
    # The tub: a faceted shell in profile, its hood and its flanks.
    tub = [(-1.75, 0.32), (1.6, 0.32), (1.95, 0.55), (1.82, 0.78), (0.75, 0.92), (0.45, 0.82), (-0.75, 0.82), (-1.1, 0.95),
           (-1.8, 0.9)]
    p.prism(tub, 0.9, (0, 0, 0), paint)
    p.prism([(0.75, 0.92), (1.82, 0.78), (1.84, 0.82), (0.78, 0.96)], 0.8, (0, 0, 0), r.choice(REPAINTS))
    for s in (-1, 1):
        p.prism([(-1.4, 0.36), (1.3, 0.36), (1.5, 0.55), (1.2, 0.72), (-1.1, 0.72), (-1.5, 0.6)], 0.1, (0, s * 0.5, 0), "Hull dark")
    p.span((1.9, -0.55, 0.36), (2.02, 0.55, 0.55), "Gunmetal")
    with p.painted():
        for i in range(4):
            p.box((0.012, 0.13, 0.15), (2.026, -0.36 + i * 0.24, 0.46), "Hazard yellow", rot=(35, 0, 0))
    _nose_lamps(p, 1.9, 0.68, 0.7, "Sodium lamp")
    # Two bucket seats, a wheel, the dash.
    for y in (-0.27, 0.27):
        p.prism([(-0.55, 0.62), (0.1, 0.62), (0.12, 0.72), (-0.45, 0.72), (-0.6, 1.3), (-0.7, 1.28)], 0.38, (0, y, 0), "Leather")
    p.span((0.5, -0.4, 0.82), (0.7, 0.4, 0.95), "Hull dark")
    p.torus(0.16, 0.018, (0.42, -0.27, 1.02), "Polymer", rot=(0, 65, 0), segments=8, sides=4)
    # The cage: a main hoop behind the seats, the screen hoop, the roof
    # bars and braces; the light bar on top.
    for x, top in ((-0.75, 1.75), (0.6, 1.62)):
        p.tube([(x, -0.5, 0.82), (x - 0.05, -0.46, top), (x - 0.05, 0.46, top), (x, 0.5, 0.82)], 0.03, "Hull dark", segments=4)
    for s in (-1, 1):
        p.tube([(-0.8, s * 0.46, 1.75), (0.55, s * 0.46, 1.62)], 0.028, "Hull dark", segments=4)
        p.tube([(-0.8, s * 0.46, 1.6), (-1.6, s * 0.42, 0.9)], 0.028, "Hull dark", segments=4)
        p.tube([(0.55, s * 0.46, 1.5), (1.4, s * 0.4, 0.9)], 0.025, "Hull dark", segments=4)
    p.tube([(-0.8, -0.46, 1.75), (0.55, 0.46, 1.62)], 0.022, "Hull dark", segments=4)
    _light_bar(p, 0.6, 1.5, 0, 1.68, lamps=4)
    # The back: water cans in a frame, the spare wheel stood up behind.
    p.span((-1.75, -0.45, 0.9), (-1.15, 0.45, 0.94), "Grating")
    for y in (-0.22, 0.22):
        _source(p, "metal_jerrycan_green", (-1.45, y, 0.94), (0, 0, 90), height=0.46)
    _balloon(p, -1.95, 0, 0.66 + 0.32, 0.32, 0.2, -1, blocks=24, both=True)
    p.collider((3.6, 1.3, 0.85), (0.05, 0, 0.32 + 0.43))
    p.turn(-1)  # nose along -Y
    return p


def rover(style: Style) -> Piece:
    """A crew rover for the long Fringe runs: a pressurised capsule on six
    balloon tyres, after the Mars rover and the explorer truck. Its nose is a
    glazed dome, its flanks have portholes and lockers, its roof carries
    solar panels, a dish and a light bar, and an airlock door and steps are
    at the back. Salvage crews and caravan escorts live in these for weeks."""
    p = Piece("rover", "vehicle", "7.6 × 3.2 × 3.6 m crew rover, front at -Y")
    p.budget = 60000
    r = rng(style, p.name)
    paint = r.choice(["Crew orange", "Repaint cream", "Hazard yellow"])
    patch = r.choice([c for c in REPAINTS if c != paint])
    # Running gear: a spine, three axles of balloon tyres on trailing arms.
    p.span((-3.2, -0.55, 0.65), (3.1, 0.55, 1.0), "Hull dark")
    for x in (2.35, 0.0, -2.35):
        for s in (-1, 1):
            _balloon(p, x, s * 1.3, 0.6, 0.6, 0.48, s, blocks=24)
            _arm(p, (x + 0.6, s * 0.6, 0.85), (x, s * 0.98, 0.6), 0.065, "Gunmetal")
            _coilover(p, (x + 0.2, s * 0.92, 0.7), (x + 0.42, s * 0.75, 1.3), radius=0.065, turns=6)
    # The capsule: a pressure hull along X, domed at the nose, banded.
    p.lathe([(0.0, -3.1), (0.9, -3.08), (1.12, -2.9), (1.15, -2.6), (1.15, 2.2), (1.1, 2.7), (0.9, 3.1), (0.55, 3.35),
             (0.0, 3.42)], (0, 0, 2.05), paint, rot=(0, 90, 0), segments=12)
    # A frame ring round the cockpit's glazing.
    p.torus(1.12, 0.045, (2.72, 0, 2.05), "Hull dark", rot=(0, 90, 0), segments=12, sides=4)
    for x in (-2.6, -1.2, 0.2, 1.6):
        p.torus(1.165, 0.04, (x, 0, 2.05), "Hull dark", rot=(0, 90, 0), segments=12, sides=4)
    # The glazed nose: a band of glass a centimetre proud of the dome.
    # The glazed nose: dark tinted glass a centimetre and a half proud of
    # the dome, all the way to its tip.
    p.lathe([(1.125, 2.74), (0.915, 3.115), (0.565, 3.365), (0.0, 3.435), (0.0, 3.45), (0.58, 3.38), (0.93, 3.13),
             (1.14, 2.75)], (0, 0, 2.05), "Screen", rot=(0, 90, 0), segments=12)
    # A patch of another paint, portholes and lockers down each flank.
    for s in (-1, 1):
        p.span((-2.1, s * 1.16 - 0.01, 1.7), (-1.0, s * 1.16 + 0.01, 2.4), patch)
        for x in (-0.6, 0.6, 1.8):
            p.cyl(0.26, 0.06, (x, s * 1.15, 2.2), "Hull dark", rot=(90, 0, 0), segments=8)
            p.cyl(0.2, 0.08, (x, s * 1.15, 2.2), "Screen", rot=(90, 0, 0), segments=8)
        # Lockers in the gaps between the wheels.
        for x in (-1.18, 1.18):
            p.span((x - 0.45, s * 0.75, 0.95), (x + 0.45, s * 1.04, 1.3), "Hull dark")
            p.span((x - 0.41, s * 1.04 if s > 0 else -1.06, 0.98), (x + 0.41, s * 1.06 if s > 0 else -1.04, 1.27), paint)
        with p.painted():
            p.stencil("12", (-1.55, s * 1.175, 2.05), 0.24, "Stencil white", facing="-Y" if s < 0 else "+Y")
    # The roof: solar panels on frames, a dish, aerials, a light bar.
    for x in (-2.2, -1.0, 0.2):
        p.span((x - 0.5, -0.8, 3.3), (x + 0.5, 0.8, 3.34), "Hull dark")
        p.span((x - 0.47, -0.77, 3.34), (x + 0.47, 0.77, 3.36), "Container blue")
        for y in (-0.6, 0.6):
            p.cyl(0.03, 0.2, (x, y, 3.2), "Hull dark", segments=4)
    p.lathe([(0.0, 0.0), (0.35, 0.12), (0.4, 0.16), (0.0, 0.06)], (1.4, 0.4, 3.35), "Hull alloy", rot=(30, 0, 0), segments=10)
    p.cyl(0.03, 0.3, (1.4, 0.4, 3.2), "Hull dark", segments=4)
    with p.plain():
        for y in (-0.7, 0.7):
            p.cyl(0.008, 1.4, (-2.9, y, 3.8), "Hull dark", segments=3)
    _light_bar(p, 2.3, 3.5, 0, 3.05, lamps=5)
    # The airlock at the back: its frame, the door, the steps down.
    p.span((-3.25, -0.6, 1.2), (-3.12, 0.6, 2.75), "Hull dark")
    p.span((-3.3, -0.5, 1.28), (-3.25, 0.5, 2.68), patch)
    p.torus(0.15, 0.02, (-3.33, 0.0, 2.0), "Steel", rot=(0, 90, 0), segments=8, sides=4)
    for z, x in ((0.95, -3.45), (0.62, -3.7), (0.3, -3.95)):
        p.span((x - 0.15, -0.45, z - 0.03), (x + 0.15, 0.45, z), "Grating")
    for s in (-1, 1):
        p.tube([(-3.25, s * 0.48, 1.1), (-4.05, s * 0.48, 0.25)], 0.03, "Hull dark", segments=4)
        p.tube([(-3.25, s * 0.55, 2.0), (-3.6, s * 0.55, 1.6), (-4.05, s * 0.55, 0.9)], 0.02, ACCENT, segments=4)
    p.collider((6.4, 2.3, 2.3), (0.1, 0, 2.05))
    p.collider((6.3, 1.1, 0.4), (-0.05, 0, 0.82))
    p.turn(-1)  # nose along -Y
    return p


# Ships -------------------------------------------------------------------------

def _ramp(p: Piece, top_y: float, top_z: float, run: float, width: float, mat: str) -> None:
    """A boarding ramp down towards -Y, from (top_y, top_z) to the ground
    run metres out: ribbed plate, side rails, hydraulic arms, and a
    collider through its top under the controller's 45°."""
    a = math.atan2(top_z, run)
    length = math.hypot(run, top_z)
    mid_y, mid_z = top_y - run / 2, top_z / 2
    t = 0.2
    deg = math.degrees(a)
    p.box((width, length, t), (0, mid_y + t / 2 * math.sin(a), mid_z - t / 2 * math.cos(a)), mat, rot=(deg, 0, 0))
    p.collider((width, length, t), (0, mid_y + t / 2 * math.sin(a), mid_z - t / 2 * math.cos(a)), rot=(deg, 0, 0))
    # Ribs across it, standing 4 cm proud for grip.
    for i in range(1, 14):
        u = i / 14
        p.box((width * 0.88, 0.08, 0.05), (0, top_y - run * u, top_z * (1 - u) + 0.025), "Hull dark", rot=(deg, 0, 0))
    for s in (-1, 1):
        x = s * (width / 2 - 0.08)
        p.box((0.2, length, 0.35), (x, mid_y, mid_z + 0.08), "Hazard yellow", rot=(deg, 0, 0))
        # Handrails on posts.
        p.tube([(s * (width / 2 - 0.1), top_y - 0.2, top_z + 1.1), (s * (width / 2 - 0.1), top_y - run + 0.4, 1.0)], 0.04, "Steel", segments=5)
        for u in (0.15, 0.5, 0.85):
            yy, zz = top_y - run * u, top_z * (1 - u)
            p.cyl(0.03, 1.0, (s * (width / 2 - 0.1), yy, zz + 0.6), "Steel", segments=5)
        # Two-part hydraulic arms from the hull to the ramp.
        p.tube([(x * 0.85, top_y - 0.3, top_z + 2.0), (x * 0.85, top_y - run * 0.25, top_z * 0.75 + 0.9)], 0.13, "Gunmetal", segments=5)
        p.tube([(x * 0.85, top_y - run * 0.22, top_z * 0.78 + 0.95), (x * 0.85, top_y - run * 0.45, top_z * 0.55 + 0.2)], 0.07, "Steel", segments=5)


def _thrusters(p: Piece, x, y, z, facing: int) -> None:
    """A cluster of four attitude thrusters on a block, nozzles out along
    facing (+1 or -1 in X)."""
    p.span((x - 0.4, y - 0.4, z - 0.4), (x + 0.4, y + 0.4, z + 0.4), "Hull dark")
    for dy, dz in ((-0.2, -0.2), (0.2, -0.2), (-0.2, 0.2), (0.2, 0.2)):
        p.lathe([(0.0, 0.0), (0.08, 0.0), (0.12, 0.12), (0.16, 0.3), (0.0, 0.25)], (x + facing * 0.4, y + dy, z + dz), "Gunmetal",
                rot=(0, 90 * facing, 0), segments=5)


def drifter_patience(style: Style) -> Piece:
    """The Patience: an independent long-haul drifter, landed on its legs at
    the Pads with its cargo bay open and the ramp down. Every player steps
    off it, in debt. Forty years of repairs show in its plating: patched,
    repainted, piped and bolted over, its engines scorched."""
    p = Piece("drifter_patience", "vehicle", "drifter freighter, about 19 × 45 × 15 m; walk up the ramp into the bay")
    p.budget = 150000
    r = rng(style, p.name)
    belly, floor = 3.0, 3.6
    y0, y1 = -16.0, 17.0  # the bay mouth and the hull's back
    bay_end = 4.0
    # The hull's cross-section, octagonal, with the bay through its front.
    outer = [(-5.5, belly), (5.5, belly), (7, 4.5), (7, 8.4), (5.5, 10), (-5.5, 10), (-7, 8.4), (-7, 4.5)]
    p.prism(outer, y1 - bay_end, (0, (bay_end + y1) / 2, 0), "Hull pale")
    length = bay_end - y0
    mid = (y0 + bay_end) / 2
    p.prism([(-5.5, belly), (5.5, belly), (6.1, floor), (-6.1, floor)], length, (0, mid, 0), "Hull pale")
    for s in (-1, 1):
        side = [(4.8, floor), (6.1, floor), (7, 4.5), (7, 8.4), (6.7, 8.8), (4.8, 8.8)]
        p.prism([(s * x, z) for x, z in side] if s > 0 else [(s * x, z) for x, z in reversed(side)], length, (0, mid, 0), "Hull pale")
    p.prism([(-6.7, 8.8), (6.7, 8.8), (5.5, 10), (-5.5, 10)], length, (0, mid, 0), "Hull pale")
    # The bay inside: a grated deck, ribs, the back bulkhead and its door,
    # lamps, pipe runs along the ceiling, cargo netted to the walls.
    p.span((-4.78, y0 + 0.02, floor - 0.02), (4.78, bay_end - 0.22, floor + 0.03), "Grating")
    p.span((-4.8, bay_end - 0.2, floor), (4.8, bay_end, 8.8), "Hull dark")
    p.span((-1.2, bay_end - 0.26, floor), (1.2, bay_end - 0.2, floor + 2.6), "Gunmetal")
    with p.painted():
        p.span((-1.2, bay_end - 0.268, floor + 2.65), (1.2, bay_end - 0.262, floor + 2.85), "Hazard yellow")
    for y in range(int(y0) + 2, int(bay_end), 3):
        p.span((-4.8, y, floor), (-4.5, y + 0.3, 8.8), "Hull dark")
        p.span((4.5, y, floor), (4.8, y + 0.3, 8.8), "Hull dark")
        p.span((-4.8, y, 8.5), (4.8, y + 0.3, 8.8), "Hull dark")
        p.span((-0.3, y + 0.05, 8.3), (0.3, y + 0.25, 8.5), "Sodium lamp")
    for x, rad, mat in ((-3.0, 0.15, "Rust"), (-2.5, 0.1, "Crew orange"), (3.0, 0.18, "Hull alloy")):
        p.tube([(x, y0 + 0.5, 8.2), (x, bay_end - 0.3, 8.2)], rad, mat, segments=5)
    for x in (-2.5, 0.0, 2.5):
        p.span((x - 0.9, bay_end - 2.0, floor + 0.03), (x + 0.9, bay_end - 0.25, floor + 1.6), r.choice(["Crew orange", "Container blue", "Container green"]))
    for s in (-1, 1):
        _hanging(p, (s * 4.42, -9.0), (s * 4.42, -3.0), 7.0, 3.7, "Tarp", ripple=0.05, thickness=0.03, cell=0.3, sag=0.25, seed=11 + s)
    # A collar round the bay's mouth, its floodlights, hazard stripes.
    p.span((-7.05, y0 - 0.4, belly), (-4.8, y0, 10.0), "Hull dark")
    p.span((4.8, y0 - 0.4, belly), (7.05, y0, 10.0), "Hull dark")
    p.span((-7.05, y0 - 0.4, 8.8), (7.05, y0, 10.05), "Hull dark")
    with p.painted():
        for i in range(10):
            z = floor + 0.4 + i * 0.5
            for s in (-1, 1):
                p.box((0.5, 0.012, 0.2), (s * 4.95, y0 - 0.41, z), "Hazard yellow", rot=(0, s * 35, 0))
    for x in (-5.8, 5.8):
        p.box((0.6, 0.4, 0.4), (x, y0 - 0.6, 9.4), "Crew grey", rot=(-25, 0, 0))
        p.box((0.5, 0.02, 0.3), (x, y0 - 0.81, 9.33), "Sodium lamp", rot=(-25, 0, 0))
    # The bridge on top at the front: a raked nose of glass in its own plane.
    p.prism([(-14.5, 10.0), (-6.0, 10.0), (-6.0, 12.3), (-11.5, 12.3), (-14.0, 11.0)], 6.0, (0, 0, 0), "Hull pale", rot=(0, 0, 90))
    p.prism([(-14.08, 11.06), (-11.56, 12.34), (-11.48, 12.2), (-13.92, 10.96)], 5.4, (0, 0, 0), "Screen", rot=(0, 0, 90))
    for s in (-1, 1):
        for i in range(4):
            y = -11.0 + i * 1.1
            p.span((s * 3.0 - 0.04, y, 10.8), (s * 3.0 + 0.04, y + 0.8, 11.6), "Screen")
    # Masts, a dish, sensor domes and aerials.
    p.cyl(0.08, 3.0, (2.0, -8.0, 13.8), "Steel", segments=5)
    for z in (13.0, 14.0, 15.0):
        p.tube([(2.0, -8.6, z), (2.0, -7.4, z)], 0.025, "Steel", segments=5)
    p.cyl(0.12, 0.8, (-2.0, -9.0, 12.7), "Hull dark", segments=5)
    p.lathe([(0.0, 0.0), (0.6, 0.12), (1.1, 0.45), (1.2, 0.6), (1.15, 0.62), (0.0, 0.25)], (-2.0, -9.0, 13.0), "Hull alloy",
            rot=(-30, 0, 0), segments=9)
    p.cyl(0.04, 0.9, (-2.0, -9.25, 13.6), "Hull dark", rot=(-30, 0, 0), segments=5)
    for x, y in ((-4.5, -6.0), (4.5, -2.0), (0.0, 12.0)):
        p.sphere(0.45, (x, y, 10.1), "Hull alloy", scale=(1, 1, 0.7), segments=7, rings=8)
        p.cyl(0.5, 0.24, (x, y, 10.0), "Hull dark", segments=7)
    # Radiator fins along the spine, behind the bridge, and the spine duct.
    for i in range(7):
        y = 1.0 + i * 2.0
        p.span((-3.5, y, 9.85), (3.5, y + 0.12, 11.6), "Hull dark")
        p.span((-3.4, y - 0.06, 11.55), (3.4, y + 0.18, 11.7), "Rust")
    p.span((-0.4, -6.0, 9.9), (0.4, 15.0, 10.7), "Hull alloy")
    for x in (-0.9, 0.9):
        p.tube([(x, -6.0, 10.25), (x, 15.0, 10.25)], 0.18, "Gunmetal" if x < 0 else "Crew orange", segments=5)
    # A cargo crane on the roof, folded: its turret, boom and hook.
    p.cyl(0.7, 0.7, (4.0, 8.0, 10.25), "Crew orange", segments=7)
    p.box((0.5, 6.0, 0.5), (4.0, 5.2, 10.9), "Crew orange", rot=(4, 0, 0))
    p.cable((4.0, 2.3, 10.7), (4.0, 2.0, 10.1), 0.02, "Hull dark", sag=0.0, segments=5, steps=3)
    p.torus(0.15, 0.04, (4.0, 2.0, 10.1), "Steel", rot=(90, 0, 0), segments=5, sides=5)
    # Cargo pods slung along both flanks, odd ones out of the paint.
    for s in (-1, 1):
        x = s * 8.2
        pod = r.choice(["Container blue", "Container green", "Container red", "Crew orange"])
        p.lathe([(0.0, -10.0), (0.6, -9.98), (0.95, -9.85), (1.2, -9.5), (1.25, -9.2), (1.25, 9.2), (1.2, 9.5), (0.95, 9.85), (0.6, 9.98), (0.0, 10.0)],
                (x, 2.0, 6.2), pod, rot=(90, 0, 0), segments=9)
        for y in (-6.0, -1.0, 4.0, 9.0):
            p.torus(1.262, 0.08, (x, y, 6.2), "Hull dark", rot=(90, 0, 0), segments=9, sides=5)
        for y in (-5.0, 7.0):
            p.span((min(s * 6.95, x), y - 0.3, 6.0), (max(s * 6.95, x), y + 0.3, 6.44), "Hull dark")
        p.collider((2.5, 20.0, 2.5), (x, 2.0, 6.2))
        # A pipe run along each flank, between the pod and the hull.
        for z, rad, mat in ((4.7, 0.12, "Rust"), (8.0, 0.09, "Hull alloy")):
            p.tube([(s * 7.2, y0 + 1.0, z), (s * 7.2, y1 - 1.0, z)], rad, mat, segments=5)
            for y in range(int(y0) + 2, int(y1), 4):
                p.span((min(s * 6.95, s * 7.4), y - 0.08, z - rad - 0.04), (max(s * 6.95, s * 7.4), y + 0.08, z + rad + 0.04), "Hull dark")
    # Attitude thrusters at the front corners.
    for s in (-1, 1):
        _thrusters(p, s * 7.4, y0 + 1.5, 9.2, s)
    # Frame rings round the hull, every few metres: hollow, so the bay
    # stays open, each edge a plate standing 25 cm proud of the hull.
    # (Their inner edges sit 10 cm inside the hull, buried in it.)
    hull = [(-5.4, belly + 0.1), (5.4, belly + 0.1), (6.9, 4.55), (6.9, 8.35), (5.4, 9.8), (-5.4, 9.8), (-6.9, 8.35), (-6.9, 4.55)]
    ring = [(-5.7, belly - 0.2), (5.7, belly - 0.2), (7.25, 4.4), (7.25, 8.5), (5.7, 10.25), (-5.7, 10.25), (-7.25, 8.5), (-7.25, 4.4)]
    for y in (-6.5, 0.5, 4.5, 8.5, 13.0):
        for i in range(8):
            j = (i + 1) % 8
            p.prism([hull[i], hull[j], ring[j], ring[i]], 0.5, (0, y, 0), "Hull dark")
    # Two outboard engines on pylons at the back, above the cargo pods:
    # intakes in front, the cowl, the nozzle and its scorched lip.
    for s in (-1, 1):
        nx, nz = s * 8.9, 9.3
        p.prism([(s * 6.9, 8.9), (s * 7.9, 8.9), (s * 7.9, 9.6), (s * 6.9, 9.8)] if s > 0 else
                [(s * 6.9, 9.8), (s * 7.9, 9.6), (s * 7.9, 8.9), (s * 6.9, 8.9)], 4.5, (0, 15.0, 0), "Hull pale")
        p.lathe([(0.0, 0.3), (0.9, 0.0), (1.25, 0.25), (1.35, 0.8), (1.35, 5.2), (1.2, 5.8), (1.05, 6.2), (1.25, 7.0), (1.15, 7.1), (0.0, 6.6)],
                (nx, 11.4, nz), "Hull pale", rot=(-90, 0, 0), segments=12)
        p.cyl(0.95, 0.06, (nx, 11.45, nz), "Hull dark", rot=(90, 0, 0), segments=10)
        p.torus(1.24, 0.08, (nx, 18.4, nz), "Rust", rot=(90, 0, 0), segments=11, sides=5)
        for y in (13.0, 15.5):
            p.torus(1.36, 0.06, (nx, y, nz), "Hull dark", rot=(90, 0, 0), segments=12, sides=5)
        with p.painted():
            p.stencil("6", (nx + s * 1.36, 14.2, nz), 0.7, "Stencil white", facing="+X" if s > 0 else "-X")
    # The engine block and its bells at the back, scorched.
    p.span((-6.0, y1, 4.0), (6.0, y1 + 2.0, 9.6), "Hull dark")
    for x, z, big in ((-3.4, 5.4, True), (3.4, 5.4, True), (0.0, 8.0, True), (0.0, 4.6, False)):
        rad = 1.6 if big else 1.3
        p.lathe([(0.0, 0.0), (0.7, 0.0), (0.75, 0.4), (0.9, 0.9), (rad * 0.85, 2.2), (rad, 3.0), (rad - 0.08, 3.05),
                 (0.0, 2.6)], (x, y1 + 2.0, z), "Gunmetal", rot=(-90, 0, 0), segments=11)
        p.torus(rad - 0.04, 0.07, (x, y1 + 5.0, z), "Rust", rot=(90, 0, 0), segments=11, sides=5)
        p.torus(0.82, 0.1, (x, y1 + 2.3, z), "Rust", rot=(90, 0, 0), segments=8, sides=5)
        for a in range(0, 360, 90):
            ca, sa = math.cos(math.radians(a)), math.sin(math.radians(a))
            p.tube([(x + ca * 1.2, y1 + 0.2, z + sa * 1.2), (x + ca * 0.8, y1 + 2.4, z + sa * 0.8)], 0.08, "Steel", segments=5)
    # Hull plates over its sides and back, patched and repainted, 2 cm proud.
    for s in (-1, 1):
        x = s * 7.0
        for i, y in enumerate(range(int(y0) + 1, int(y1) - 1, 3)):
            for j, (z0, z1) in enumerate(((4.6, 6.4), (6.5, 8.3))):
                if (s, i, j) in ((1, 2, 1), (-1, 3, 0)):
                    continue  # the pipe brackets' ends, and where a plate came off
                mat = r.choice(REPAINTS) if r.random() < style.repaint + 0.15 else "Hull pale"
                p.span((min(x, x + s * 0.02), y + 0.05, z0), (max(x, x + s * 0.02), y + 2.9, z1), mat)
        with p.painted():
            p.span((min(s * 7.026, s * 7.034), y0, 4.45), (max(s * 7.026, s * 7.034), y1, 4.55), "Crew orange")
    for y in range(int(y0) + 1, int(y1) - 1, 4):
        mat = r.choice(REPAINTS) if r.random() < style.repaint else "Hull pale"
        p.span((-4.5, y + 0.1, 9.95), (4.5, y + 3.8, 10.06), mat)
    # Its registration, big on both flanks; it was never given letters.
    with p.painted():
        p.stencil("60-114", (7.03, -11.8, 6.6), 1.1, "Stencil white", facing="+X")
        p.stencil("60-114", (-7.03, -11.8, 6.6), 1.1, "Stencil white", facing="-X")
        p.span((-0.8, y0 - 0.41, 9.1), (0.8, y0 - 0.406, 9.7), "Stencil white")
    # Landing legs: an upper strut and a two-part ram to a broad foot, the
    # foot's pad, and a chain to a ground stake.
    for x in (-6.0, 6.0):
        for y in (-11.0, 12.0):
            s = 1 if x > 0 else -1
            fx = x + s * 1.6
            p.tube([(x * 0.9, y, belly + 0.2), (fx, y, 0.6)], 0.35, "Hull dark", segments=6)
            p.tube([(x * 0.7, y, belly), ((x * 0.7 + fx) / 2, y, (belly + 0.6) / 2)], 0.2, "Gunmetal", segments=5)
            p.tube([((x * 0.7 + fx) / 2, y, (belly + 0.6) / 2 + 0.05), (fx, y, 0.6)], 0.13, "Steel", segments=5)
            p.lathe([(0.0, 0.0), (1.05, 0.0), (1.0, 0.25), (0.6, 0.45), (0.0, 0.5)], (fx, y, 0.0), "Hull dark", segments=9)
            p.collider((2.0, 2.0, 0.6), (fx, y, 0.3))
            p.collider((0.7, 0.7, belly), (x + s * 0.8, y, belly / 2))
            p.cable((fx + s * 0.6, y, 0.3), (fx + s * 2.0, y + 0.8, 0.05), 0.04, "Hull dark", sag=0.05, segments=5, steps=4)
            p.cyl(0.07, 0.4, (fx + s * 2.0, y + 0.8, 0.15), "Steel", segments=5)
    _ramp(p, y0, floor, 6.5, 8.0, "Hull alloy")
    # Colliders: the bay's floor, walls and roof, the hull behind it, the
    # bridge and the engines.
    p.collider((9.6, length, floor - belly), (0, mid, (belly + floor) / 2))
    for s in (-1, 1):
        p.collider((2.2, length, 8.8 - belly), (s * 5.9, mid, (belly + 8.8) / 2))
    p.collider((14.0, length, 1.2), (0, mid, 9.4))
    p.collider((14.0, y1 - bay_end, 10.0 - belly), (0, (bay_end + y1) / 2, (belly + 10.0) / 2))
    p.collider((6.0, 8.5, 2.3), (0, -10.25, 11.15))
    p.collider((12.0, 2.0, 5.6), (0, y1 + 1.0, 6.8))
    return p


def receivership_shuttle(style: Style) -> Piece:
    """The Corvane Receivership's lander: white and blue, every line
    straight, not a scratch on it (CLEAN in finish.py keeps it so). It comes
    down in Act 3 and looks wrong on the Red, which is the point. Its side
    hatch lowers a ramp."""
    p = Piece("receivership_shuttle", "vehicle", "Corvane lander, about 12 × 16 × 9 m; side ramp at +X")
    p.budget = 60000
    belly = 2.0
    w = 2.8
    # The body: a long wedge, nose along -Y, cut from side to side.
    side = [(-8.0, 3.0), (-6.5, belly), (6.5, belly), (7.8, 2.7), (7.8, 5.6), (5.0, 6.4), (-2.5, 6.4), (-6.8, 4.6)]
    p.prism(side, 2 * w, (0, 0, 0), "Corvane white", rot=(0, 0, 90))
    # The canopy: dark glass 1.5 cm proud of the nose's slope (out along
    # the slope's normal), narrower than the body.
    dy, dz = -2.5 - -6.8, 6.4 - 4.6
    n = math.hypot(dy, dz)
    ny, nz = -dz / n * 0.015, dy / n * 0.015
    canopy = [(-6.62, 4.68), (-2.75, 6.3), (-3.1, 6.1), (-6.3, 4.76)]
    p.prism([(y + ny, z + nz) for y, z in canopy], 2 * w - 0.7, (0, 0, 0), "Screen", rot=(0, 0, 90))
    # The blue band, the windows and the panel lines, painted.
    with p.painted():
        for s in (-1, 1):
            x = s * (w + 0.006)
            fc = "-X" if s < 0 else "+X"
            p.span((min(x, x + s * 0.004), -6.8, 3.0), (max(x, x + s * 0.004), 7.8, 3.4), "Corvane blue")
            for i in range(6):
                y = -3.5 + i * 1.2
                p.span((min(x, x + s * 0.004), y, 4.3), (max(x, x + s * 0.004), y + 0.7, 4.8), "Screen")
            for y in (-5.0, -1.8, 1.4, 4.6):
                xl = s * (w + 0.016)
                p.span((min(xl, xl + s * 0.004), y, 2.1), (max(xl, xl + s * 0.004), y + 0.02, 6.2), "Hull pale")
            p.stencil("90", (s * (w + 0.008), 3.6, 5.3), 0.6, "Corvane blue", facing=fc)
        for y in (-4.0, 0.0, 4.0):
            p.span((-w + 0.2, y, 6.406), (w - 0.2, y + 0.02, 6.41), "Hull pale")
        # The company's mark on the roof: a ring round a bar.
        p.torus(0.8, 0.08, (0, 2.0, 6.42), "Corvane blue", segments=12, sides=5)
        p.span((-0.6, 1.92, 6.4), (0.6, 2.08, 6.43), "Corvane blue")
    # Stub wings with engine pods: intakes in front, nozzles behind with
    # their blue glow set back inside.
    for s in (-1, 1):
        p.prism([(0.0, 0.0), (2.6, 0.6), (2.6, 3.4), (0.0, 5.0)], 0.25, (s * w, 1.5, 3.6), "Corvane white",
                rot=(90, 0, 0 if s > 0 else 180))
        px = s * (w + 2.2)
        p.lathe([(0.0, -1.9), (0.45, -1.9), (0.6, -1.6), (0.62, -1.2), (0.62, 1.5), (0.55, 1.9), (0.0, 1.9)], (px, 4.2, 3.6), "Corvane white",
                rot=(90, 0, 0), segments=10)
        p.torus(0.5, 0.06, (px, 2.33, 3.6), "Corvane blue", rot=(90, 0, 0), segments=9, sides=5)
        p.lathe([(0.0, 0.0), (0.5, 0.0), (0.56, 0.3), (0.52, 0.5), (0.0, 0.2)], (px, 6.0, 3.6), "Gunmetal", rot=(-90, 0, 0), segments=9)
        p.cyl(0.38, 0.05, (px, 6.15, 3.6), "Status blue", rot=(90, 0, 0), segments=9)
        p.cyl(0.4, 0.03, (px, 2.29, 3.6), "Hull dark", rot=(90, 0, 0), segments=9)
    p.prism([(4.0, 6.4), (7.6, 6.4), (7.8, 8.6), (6.6, 8.6)], 0.25, (0, 0, 0), "Corvane white", rot=(0, 0, 90))
    with p.painted():
        p.prism([(6.5, 7.6), (7.68, 7.6), (7.75, 8.1), (6.62, 8.1)], 0.265, (0, 0, 0), "Corvane blue", rot=(0, 0, 90))
    # The main engines across its tail.
    for x in (-1.2, 0.0, 1.2):
        p.lathe([(0.0, 0.0), (0.42, 0.0), (0.5, 0.2), (0.55, 0.4), (0.0, 0.15)], (x, 7.8, 4.1), "Gunmetal", rot=(-90, 0, 0), segments=8)
        p.cyl(0.3, 0.04, (x, 7.95, 4.1), "Status blue", rot=(90, 0, 0), segments=8)
    # Landing legs: a ram in its sleeve, a knee, square feet.
    for x in (-w + 0.4, w - 0.4):
        for y in (-4.5, 5.5):
            p.span((x - 0.18, y - 0.18, 1.1), (x + 0.18, y + 0.18, belly), "Corvane white")
            p.cyl(0.1, 0.9, (x, y, 0.7), "Steel", segments=6)
            p.span((x - 0.5, y - 0.5, 0.0), (x + 0.5, y + 0.5, 0.2), "Corvane blue")
            p.span((x - 0.2, y - 0.2, 0.2), (x + 0.2, y + 0.2, 0.32), "Gunmetal")
            p.collider((1.0, 1.0, belly), (x, y, belly / 2))
    # The side hatch on +X and its ramp down to the ground, rails and arms.
    p.span((w + 0.012, -1.2, belly + 0.2), (w + 0.042, 1.2, 4.6), "Hull dark")
    p.span((w + 0.042, -1.0, belly + 0.2), (w + 0.062, 1.0, 4.4), "Corvane white")
    run = 3.6
    a = math.atan2(belly + 0.2, run)
    length = math.hypot(run, belly + 0.2)
    t = 0.15
    cx, cz = w + run / 2 + t / 2 * math.sin(a), (belly + 0.2) / 2 - t / 2 * math.cos(a)
    p.box((length, 2.0, t), (cx, 0, cz), "Corvane white", rot=(0, math.degrees(a), 0))
    p.collider((length, 2.0, t), (cx, 0, cz), rot=(0, math.degrees(a), 0))
    for s in (-1, 1):
        p.box((length, 0.08, 0.12), (w + run / 2, s * 1.0, (belly + 0.2) / 2 + 0.05), "Corvane blue", rot=(0, math.degrees(a), 0))
        p.tube([(w + 0.2, s * 1.0, belly + 1.3), (w + run - 0.3, s * 1.0, 1.0)], 0.035, "Steel", segments=5)
        p.tube([(w + 0.05, s * 0.9, belly + 1.2), (w + 1.2, s * 0.9, belly - 0.3)], 0.06, "Steel", segments=5)
    p.collider((2 * w, 14.5, 4.4), (0, 0.4, belly + 2.2))
    return p


def covered_car(style: Style) -> Piece:
    """A car under a tarp, parked and left: someone's Earth runabout, kept
    for a better day. Dressing for the Pads and the Fringe. (Poly Haven's
    covered_car.)"""
    p = Sourced("covered_car", "prop", "car under a tarp, 1.8 × 4.4 × 1.4 m", asset="covered_car", res="2k", width=4.4, budget=20000)
    p.collider((1.7, 4.2, 1.3), (0, 0, 0.65))
    return p


PIECES = [hauler, hauler_tanker, bike, trike, buggy, rover, drifter_patience, receivership_shuttle, covered_car]
