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


# Wheels ----------------------------------------------------------------------

def _wheel(p: Piece, x, y, z, radius, width, side: int, lugs: int = 8, rim="Hull alloy", hub="Hull dark") -> None:
    """A wheel on an axle along Y, its outer face towards side (+1 or -1 in
    Y): a tyre with bulged walls and a lugged tread, a pressed rim with its
    bolts and a hub."""
    r, w = radius, width
    rr = 0.6 * r
    rot = (-90, 0, 0) if side > 0 else (90, 0, 0)  # the lathe's axis out along side
    p.lathe([(rr, -w / 2 + 0.03), (0.86 * r, -w / 2), (r, -0.3 * w), (r, 0.3 * w), (0.86 * r, w / 2), (rr, w / 2 - 0.03)],
            (x, y, z), "Tyre", rot=rot, segments=9)
    # The tread: lugs in two staggered rows, sunk into the tyre.
    for i in range(lugs):
        a = math.tau * i / lugs
        off = (0.2 if i % 2 else -0.2) * w
        c = (x + math.cos(a) * (r + 0.008), y + off, z + math.sin(a) * (r + 0.008))
        p.box((0.32 * math.tau * r / lugs, 0.42 * w, 0.045), c, "Tyre", rot=(0, 90 - math.degrees(a), 0))
    # The rim: a dished face 8 mm in front of the tyre's, the bolts, the hub.
    face = y + side * (w / 2 - 0.022)
    p.cyl(rr * 0.98, 0.03, (x, face - side * 0.015, z), rim, rot=(90, 0, 0), segments=9)
    p.lathe([(0.0, 0.0), (rr * 0.42, 0.0), (rr * 0.38, 0.03), (rr * 0.2, 0.05), (0.0, 0.055)], (x, face, z), hub, rot=rot, segments=6)
    # Its bolts, painted on (baked into the rim's texture).
    with p.painted():
        for i in range(6):
            a = math.tau * i / 6
            p.cyl(0.018 * r / 0.55, 0.012, (x + math.cos(a) * rr * 0.55, face + side * 0.012, z + math.sin(a) * rr * 0.55),
                  "Steel", rot=(90, 0, 0), segments=5)


def _spoked(p: Piece, x, z, radius, width, knobs: int = 12) -> None:
    """A bike's spoked wheel on an axle along Y, centred on y = 0: a
    knobbly tyre, a rim, spokes laced to a hub, and a brake disc."""
    r = radius
    p.torus(r - 0.045, 0.045, (x, 0, z), "Tyre", rot=(90, 0, 0), segments=13, sides=5)
    for i in range(knobs):
        a = math.tau * i / knobs
        off = 0.022 if i % 2 else -0.022
        p.box((0.032, 0.035, 0.03), (x + math.cos(a) * (r - 0.004), off, z + math.sin(a) * (r - 0.004)), "Tyre",
              rot=(0, 90 - math.degrees(a), 0))
    p.torus(r - 0.095, 0.014, (x, 0, z), "Steel", rot=(90, 0, 0), segments=13, sides=5)
    p.lathe([(0.0, -0.05), (0.035, -0.05), (0.04, -0.035), (0.03, 0.0), (0.04, 0.035), (0.035, 0.05), (0.0, 0.05)],
            (x, 0, z), "Hull alloy", rot=(90, 0, 0), segments=6)
    for i in range(12):
        a = math.tau * i / 12
        side = 0.035 if i % 2 else -0.035
        p.tube([(x + math.cos(a + 0.2) * 0.035, side, z + math.sin(a + 0.2) * 0.035),
                (x + math.cos(a) * (r - 0.1), 0, z + math.sin(a) * (r - 0.1))], 0.0025, "Steel", segments=5)
    p.cyl(0.11, 0.006, (x, -0.062, z), "Steel", rot=(90, 0, 0), segments=10)
    with p.painted():
        for i in range(6):
            a = math.tau * i / 6
            p.cyl(0.012, 0.008, (x + math.cos(a) * 0.075, -0.07, z + math.sin(a) * 0.075), "Hull dark", rot=(90, 0, 0), segments=5)


# Hauler: a six-wheeled truck, made with its cab at +X and its bed behind -------

def _leaf_spring(p: Piece, x, y) -> None:
    """A leaf spring under the frame rail at (x, y): its stack of leaves,
    each shorter than the one above, as one stepped plate."""
    halves = (0.55, 0.47, 0.38, 0.29)
    right = []
    for i, h in enumerate(halves):
        z = 0.62 - 0.022 * (i + 1)
        right.append((x + h, z))
        if i + 1 < len(halves):
            right.append((x + halves[i + 1], z))
    left = [(2 * x - px, pz) for px, pz in reversed(right)]
    p.prism([(x - 0.55, 0.62), (x + 0.55, 0.62)] + right + left, 0.09, (0, y, 0), "Hull dark")


def _hauler_chassis(p: Piece, style: Style) -> tuple[str, str]:
    """The chassis, suspension, wheels and cab; returns its two paints."""
    r = rng(style, p.name)
    paint = r.choice(["Crew orange", "Repaint teal", "Repaint oxide", "Repaint cream"])
    patch = r.choice([c for c in REPAINTS if c != paint])
    w = 1.3
    # Frame rails, cross members and the bumpers.
    for y in (-0.55, 0.55):
        p.span((-4.0, y - 0.1, 0.62), (3.9, y + 0.1, 0.9), "Hull dark")
        with p.painted():
            for i in range(26):
                p.cyl(0.012, 0.012, (-3.8 + i * 0.3, y + (0.106 if y > 0 else -0.106), 0.76), "Steel", rot=(90, 0, 0), segments=5)
    for x in (-3.8, -2.2, -0.6, 1.0, 2.6, 3.7):
        p.span((x - 0.06, -0.45, 0.68), (x + 0.06, 0.45, 0.84), "Hull dark")
    p.span((3.9, -w, 0.5), (4.12, w, 0.86), "Hull dark")
    p.span((-4.1, -w + 0.1, 0.58), (-3.98, w - 0.1, 0.86), "Gunmetal")
    with p.painted():
        for i in range(8):
            y0 = -w + 0.15 + i * 0.3
            p.box((0.012, 0.14, 0.28), (-4.106, y0 + 0.07, 0.72), "Hazard yellow", rot=(35, 0, 0))
    # Tow hooks at the front.
    for x, s in ((4.18, 1),):
        for y in (-0.6, 0.6):
            p.torus(0.07, 0.02, (x, y, 0.62), "Steel", rot=(0, 0, 0), segments=5, sides=5)
    # Axles, springs, wheels, the rear axles' differentials.
    for x in (2.9, -1.6, -2.9):
        p.cyl(0.06, 2.1, (x, 0, 0.55), "Gunmetal", rot=(90, 0, 0), segments=5)
        for y in (-0.55, 0.55):
            _leaf_spring(p, x, y)
        for y in (-1.05, 1.05):
            _wheel(p, x, y, 0.55, 0.55, 0.42, 1 if y > 0 else -1)
    for x in (-1.6, -2.9):
        p.lathe([(0.0, -0.18), (0.16, -0.17), (0.22, -0.05), (0.2, 0.08), (0.12, 0.17), (0.0, 0.18)], (x, 0, 0.55), "Gunmetal",
                rot=(0, 90, 0), segments=7)
    p.tube([(-1.6, 0, 0.55), (0.0, 0, 0.66), (2.6, 0, 0.7)], 0.05, "Gunmetal", segments=5)
    # Mudguards over the wheels, and mud flaps behind them.
    for x0, x1 in ((2.28, 3.53), (-3.55, -0.95)):
        for s in (-1, 1):
            y0, y1 = sorted((s * 0.82, s * (w + 0.02)))
            p.span((x0, y0, 1.2), (x1, y1, 1.27), "Hull dark")
            p.span((x0 - 0.02, y0, 0.96), (x0 + 0.04, y1, 1.2), "Hull dark")
    for x in (2.25, -3.6):
        for s in (-1, 1):
            y0, y1 = sorted((s * 0.84, s * 1.26))
            p.span((x - 0.015, y0, 0.25), (x + 0.015, y1, 1.0), "Rubber")
    # The fuel tank on the right, an air tank and a battery box on the left,
    # steps up to the doors.
    p.lathe([(0.0, 0.0), (0.26, 0.0), (0.29, 0.03), (0.29, 0.57), (0.26, 0.6), (0.0, 0.6)], (1.15, -0.95, 0.82), "Steel",
            rot=(0, 90, 0), segments=8)
    for x in (1.3, 1.6):
        p.torus(0.296, 0.012, (x, -0.95, 0.82), "Hull dark", rot=(0, 90, 0), segments=8, sides=5)
    p.cyl(0.05, 0.04, (1.45, -0.95, 1.12), "Hull dark", segments=5)
    p.cyl(0.16, 0.6, (1.55, 0.85, 0.85), "Gunmetal", rot=(0, 90, 0), segments=6)
    p.span((1.1, 0.75, 0.62), (1.55, 1.15, 1.0), "Hull dark")
    p.span((1.12, 0.73, 0.98), (1.53, 1.17, 1.04), paint)
    for s in (-1, 1):
        # Steps up to the door, behind the front wheel.
        y0, y1 = sorted((s * 1.0, s * 1.32))
        b0, b1 = sorted((s * 0.98, s * 1.34))
        for z in (0.5, 0.74):
            p.span((1.88, y0, z), (2.26, y1, z + 0.04), "Grating")
        p.span((1.85, b0, 0.46), (1.89, b1, 0.92), "Hull dark")
        p.span((2.25, b0, 0.46), (2.29, b1, 0.92), "Hull dark")
    # The cab: lower body, rear wall, roof on pillars; glass in its own
    # plane between them, seats and a wheel seen through it.
    p.span((2.2, -w, 0.92), (3.9, w, 1.95), paint)
    p.span((2.2, -w, 1.95), (2.32, w, 2.98), paint)
    p.span((2.2, -w, 2.98), (3.62, w, 3.18), paint)
    with p.painted():
        p.span((2.21, -w - 0.005, 1.93), (3.89, -w + 0.002, 1.97), "Hull dark")
    slope = math.degrees(math.atan2(0.4, 1.03))
    for s in (-1, 1):
        # Windscreen pillars, raked, and the rear pillars.
        p.box((0.1, 0.12, 1.12), (3.76, s * (w - 0.06), 2.47), paint, rot=(0, -slope, 0))
        p.span((2.32, s * w - 0.06, 1.95), (2.44, s * w + 0.0, 2.98) if s > 0 else (2.44, -w + 0.06, 2.98), paint)
    p.box((0.08, 0.1, 1.08), (3.76, 0, 2.47), paint, rot=(0, -slope, 0))
    for s in (-1, 1):
        p.box((0.02, w - 0.26, 1.06), (3.72, s * 0.59, 2.47), "Glass", rot=(0, -slope, 0))
        p.prism([(2.44, 1.95), (3.7, 1.95), (3.38, 2.98), (2.44, 2.98)], 0.02, (0, s * (w - 0.05), 0), "Glass")
        # The doors, proud of the body, handles, hinges and mirrors.
        y = s * (w + 0.008)
        p.span((2.48, min(y, s * w), 0.98), (3.5, max(y, s * w), 1.9), patch)
        p.span((3.2, min(y, y + s * 0.03), 1.6), (3.38, max(y, y + s * 0.03), 1.66), "Hull dark")
        for z in (1.15, 1.7):
            p.cyl(0.025, 0.12, (3.52, y, z), "Steel", segments=5)
        p.tube([(3.75, s * w, 2.3), (3.75, s * (w + 0.35), 2.4), (3.75, s * (w + 0.35), 2.7)], 0.02, "Hull dark", segments=5)
        p.span((3.72, s * (w + 0.29), 2.45), (3.8, s * (w + 0.47), 2.85), "Hull dark")
        p.span((3.81, s * (w + 0.3), 2.47), (3.82, s * (w + 0.46), 2.83), "Glass")
        # Headlights in pressed housings.
        p.lathe([(0.0, 0.0), (0.15, 0.0), (0.16, 0.05), (0.15, 0.12), (0.0, 0.12)], (3.91, s * 0.95, 1.2), "Steel", rot=(0, 90, 0), segments=7)
        p.cyl(0.12, 0.02, (4.035, s * 0.95, 1.2), "Sodium lamp", rot=(0, 90, 0), segments=7)
        p.cyl(0.05, 0.03, (3.95, s * 1.18, 0.98), "Fabric red", rot=(0, 90, 0), segments=5)
    # Seats and the wheel inside.
    for y in (-0.6, 0.6):
        p.span((2.45, y - 0.28, 1.95), (2.95, y + 0.28, 2.1), "Leather")
        p.span((2.4, y - 0.28, 2.1), (2.52, y + 0.28, 2.75), "Leather")
    p.span((3.35, -w + 0.1, 1.95), (3.7, w - 0.1, 2.2), "Hull dark")
    p.torus(0.2, 0.02, (3.25, 0.6, 2.35), "Polymer", rot=(0, 60, 0), segments=8, sides=5)
    p.cyl(0.03, 0.4, (3.38, 0.6, 2.2), "Hull dark", rot=(0, 60, 0), segments=5)
    # The grille: a frame and its slats; a bull bar in front.
    p.span((3.9, -0.82, 1.0), (3.96, 0.82, 1.9), "Hull dark")
    for z in (1.08, 1.22, 1.36, 1.5, 1.64, 1.78):
        p.box((0.05, 1.56, 0.07), (3.985, 0, z), "Steel", rot=(0, -25, 0))
    p.tube([(4.22, -1.0, 0.6), (4.22, -1.0, 1.7), (4.22, 1.0, 1.7), (4.22, 1.0, 0.6)], 0.055, "Steel", segments=5)
    p.tube([(4.22, -0.5, 0.7), (4.22, -0.5, 1.7)], 0.04, "Steel", segments=5)
    p.tube([(4.22, 0.5, 0.7), (4.22, 0.5, 1.7)], 0.04, "Steel", segments=5)
    for y in (-0.8, 0.8):
        p.tube([(4.12, y, 1.2), (4.22, y, 1.2)], 0.04, "Steel", segments=5)
    # The roof: a rack, a light bar, a beacon, the horn and an aerial.
    for y in (-1.0, 1.0):
        p.tube([(2.3, y, 3.32), (3.5, y, 3.32)], 0.025, "Hull dark", segments=5)
        for x in (2.3, 3.5):
            p.cyl(0.02, 0.14, (x, y, 3.25), "Hull dark", segments=5)
    for x in (2.6, 2.9, 3.2):
        p.tube([(x, -1.0, 3.32), (x, 1.0, 3.32)], 0.02, "Hull dark", segments=5)
    p.span((3.52, -0.9, 3.18), (3.62, 0.9, 3.3), "Hull dark")
    for y in (-0.6, -0.2, 0.2, 0.6):
        p.cyl(0.07, 0.02, (3.63, y, 3.24), "Sodium lamp", rot=(0, 90, 0), segments=5)
    p.lathe([(0.0, 0.0), (0.08, 0.0), (0.07, 0.1), (0.0, 0.12)], (2.6, 0, 3.18), "Hazard yellow", segments=5)
    p.lathe([(0.0, 0.0), (0.03, 0.0), (0.08, 0.25), (0.0, 0.25)], (3.0, 1.1, 3.25), "Brass", rot=(0, 90, 0), segments=5)
    p.cyl(0.006, 1.2, (2.35, -1.2, 3.75), "Hull dark", segments=5)
    # The exhaust stack up the back of the cab, its heat shield and cap.
    p.cyl(0.08, 2.4, (2.12, 1.1, 2.2), "Rust", segments=6)
    p.cyl(0.098, 0.8, (2.12, 1.1, 2.4), "Steel", segments=6)
    p.box((0.2, 0.2, 0.02), (2.12, 1.1, 3.42), "Hull dark", rot=(0, 20, 0))
    # Welded patches, a stencilled number, rust run from the seams.
    p.span((2.25, -w - 0.008, 1.32), (2.42, -w, 1.72), "Rust")
    with p.painted():
        p.stencil("27", (2.95, -w - 0.016, 1.62), 0.26, "Stencil white")
        p.stencil("27", (2.95, w + 0.016, 1.62), 0.26, "Stencil white", facing="+Y")
        for x in (2.6, 3.42):
            p.span((x, -w - 0.016, 1.3), (x + 0.04, -w - 0.014, 1.9), "Rust")
    p.collider((1.85, 2 * w, 2.55), (3.07, 0, 0.6 + 2.55 / 2))
    return paint, patch


def hauler(style: Style) -> Piece:
    """A six-wheeled cargo hauler: a patched cab, and a planked flatbed
    whose load is under a tarp and ratchet straps. The Pads-to-Hull haul
    contracts' workhorse; bought, or given as a faction's favour."""
    p = Piece("hauler", "vehicle", "8.3 × 2.6 × 3.4 m hauler, cab at -Y")
    p.budget = 60000
    paint, patch = _hauler_chassis(p, style)
    r = rng(style, p.name + "bed")
    w = 1.3
    # The bed: a steel deck under wooden planks, stake pockets and rails.
    p.span((-4.0, -w, 0.9), (2.1, w, 1.08), "Hull alloy")
    for i in range(10):
        y0 = -w + 0.02 + i * 0.26
        p.span((-3.98, y0, 1.08), (2.08, y0 + 0.24, 1.1), r.choice(["Wood", "Wood", "Wood dark"]))
    for s in (-1, 1):
        y0, y1 = sorted((s * (w - 0.04), s * (w + 0.04)))
        p.span((-4.02, y0, 1.0), (2.12, y1, 1.16), paint)
        for x in (-3.6, -2.4, -1.2, 0.0, 1.2):
            p.span((x - 0.05, y0 - 0.01, 1.16), (x + 0.05, y1 + 0.01, 1.3), "Hull dark")
    # The headboard: a frame with a mesh between.
    p.tube([(1.95, -w + 0.05, 1.1), (1.95, -w + 0.05, 2.7), (1.95, w - 0.05, 2.7), (1.95, w - 0.05, 1.1)], 0.04, "Hull dark", segments=5)
    p.span((1.93, -w + 0.1, 1.15), (1.97, w - 0.1, 2.65), "Grating")
    p.collider((6.1, 2 * w, 0.7), (-0.95, 0, 0.6 + 0.35))
    # The load: crates two high, the front of it under a tarp, the back
    # open, everything ratchet-strapped down.
    stacks = [(-3.2, 1.0), (-1.9, 1.15), (-0.6, 1.15), (0.7, 1.1)]
    top = 0
    for x, h in stacks:
        for y in (-0.62, 0.62):
            hh = h * r.uniform(0.85, 1.0)
            mat = r.choice(["Crew orange", "Container blue", "Container green", "Repaint cream"])
            p.span((x - 0.6, y - 0.56, 1.1), (x + 0.6, y + 0.56, 1.1 + hh), mat)
            p.span((x - 0.62, y - 0.58, 1.1 + hh - 0.08), (x + 0.62, y + 0.58, 1.1 + hh + 0.012), "Hull dark")
            top = max(top, hh)
    lid = 1.1 + top + 0.05
    p.cloth([(-2.55, -w - 0.06, lid), (1.4, -w - 0.06, lid), (1.4, w + 0.06, lid), (-2.55, w + 0.06, lid)],
            "Tarp", sag=-0.04, ripple=0.03, thickness=0.012, cell=0.3, seed=3, droop=(0.2, 0, 0.2, 0))
    for s in (-1, 1):
        _hanging(p, (-2.55, s * (w + 0.065)), (1.4, s * (w + 0.065)), lid, 1.45, "Tarp", ripple=0.025, thickness=0.014, cell=0.3, seed=5 + s)
    for x in (-2.0, -0.6, 0.9, -3.2):
        p.tube([(x, -w - 0.09, 1.2), (x, -w - 0.09, lid + 0.04), (x, w + 0.09, lid + 0.04), (x, w + 0.09, 1.2)], 0.016,
               "Hazard yellow", segments=5)
        p.span((x - 0.05, -w - 0.13, 1.3), (x + 0.05, -w - 0.09, 1.42), "Steel")
    p.collider((5.6, 2 * w - 0.2, top), (-1.3, 0, 1.1 + top / 2))
    p.turn(-1)  # nose along -Y
    return p


def hauler_tanker(style: Style) -> Piece:
    """The hauler with a tank on its back: water out to the Fringe, fuel to
    the Pads. A walkway and rails along the top, manholes, a ladder, a hose
    reel and valves at the back. Whoever drives it is carrying the colony's
    leverage."""
    p = Piece("hauler_tanker", "vehicle", "8.5 × 2.6 × 3.5 m tanker, cab at -Y")
    p.budget = 60000
    paint, patch = _hauler_chassis(p, style)
    # The tank on its cradles, banded, domed at its ends.
    p.span((-4.0, -1.0, 0.9), (2.1, 1.0, 1.05), "Hull dark")
    p.lathe([(0.0, -3.08), (0.6, -3.05), (0.92, -2.95), (1.05, -2.8), (1.05, 2.8), (0.92, 2.95), (0.6, 3.05), (0.0, 3.08)],
            (-1.0, 0, 2.15), "Water blue", rot=(0, 90, 0), segments=11)
    for x in (-3.4, -1.9, -0.1, 1.5):
        p.torus(1.058, 0.04, (x, 0, 2.15), "Hull dark", rot=(0, 90, 0), segments=11, sides=5)
        p.span((x - 0.1, -0.85, 1.05), (x + 0.1, 0.85, 1.4), "Hull dark")
    # The walkway on top, its rails, two manholes.
    p.span((-3.6, -0.3, 3.19), (1.6, 0.3, 3.25), "Grating")
    for s in (-1, 1):
        for x in (-3.4, -2.0, -0.6, 0.8):
            p.cyl(0.018, 0.9, (x, s * 0.36, 3.65), "Steel", segments=5)
        p.tube([(-3.4, s * 0.36, 4.1), (0.8, s * 0.36, 4.1)], 0.02, "Steel", segments=5)
    for x in (-2.6, -0.2):
        p.lathe([(0.0, 0.0), (0.26, 0.0), (0.26, 0.1), (0.22, 0.14), (0.0, 0.15)], (x, 0, 3.15), "Hull alloy", segments=8)
        p.torus(0.2, 0.02, (x, 0, 3.32), "Steel", segments=6, sides=5)
    # The ladder at the back, the hose reel and valves.
    for y in (0.5, 0.9):
        p.tube([(-4.12, y, 1.1), (-4.12, y, 3.25)], 0.025, "Steel", segments=5)
    for z in (1.4, 1.75, 2.1, 2.45, 2.8, 3.15):
        p.tube([(-4.12, 0.5, z), (-4.12, 0.9, z)], 0.018, "Steel", segments=5)
    p.cyl(0.3, 0.32, (-3.8, -0.6, 1.4), "Crew orange", rot=(0, 90, 0), segments=8)
    p.cyl(0.24, 0.34, (-3.8, -0.6, 1.4), "Rubber", rot=(0, 90, 0), segments=8)
    p.cable((-3.96, -0.6, 1.2), (-4.1, -0.9, 0.3), 0.035, "Rubber", sag=0.1, segments=5)
    for y in (-0.1, 0.2):
        p.lathe([(0.0, 0.0), (0.05, 0.0), (0.05, 0.2), (0.08, 0.22), (0.08, 0.3), (0.0, 0.3)], (-4.08, y, 1.3), "Brass", rot=(0, -90, 0), segments=5)
        p.torus(0.07, 0.012, (-4.4, y, 1.3), "Fabric red", rot=(0, 90, 0), segments=5, sides=5)
    # Hazard placards, a number and a band.
    with p.painted():
        p.stencil("40", (-2.2, -1.068, 2.7), 0.32, "Stencil white")
        for x in (-3.0, 0.8):
            p.box((0.32, 0.01, 0.32), (x, -1.06, 2.0), "Hazard yellow", rot=(0, 45, 0))
    p.collider((6.1, 2.1, 2.3), (-0.95, 0, 0.9 + 2.3 / 2))
    p.turn(-1)  # nose along -Y
    return p


# Bike and trike --------------------------------------------------------------

def bike(style: Style) -> Piece:
    """A rugged dirt bike: spoked wheels on knobbly tyres, long-travel
    forks, a finned single-cylinder engine, a patched tank, and panniers
    for courier work. Bought, or won."""
    p = Piece("bike", "vehicle", "2.1 × 0.8 × 1.2 m dirt bike, front at -Y")
    p.budget = 30000
    r = rng(style, p.name)
    paint = r.choice(["Crew orange", "Repaint teal", "Hazard yellow", "Repaint oxide"])
    for x in (-0.72, 0.72):
        _spoked(p, x, 0.34, 0.34, 0.13)
    # Frame: a cradle under the engine, the backbone, the swingarm.
    p.tube([(0.38, 0, 0.86), (0.3, 0, 0.6), (0.1, 0, 0.3), (-0.2, 0, 0.28), (-0.25, 0, 0.5)], 0.022, "Hull dark", segments=5)
    p.tube([(0.38, 0, 0.88), (0.05, 0, 0.82), (-0.25, 0, 0.6), (-0.6, 0, 0.78)], 0.026, "Hull dark", segments=5)
    for s in (-1, 1):
        p.tube([(-0.2, s * 0.07, 0.42), (-0.72, s * 0.07, 0.34)], 0.022, "Gunmetal", segments=5)
    # The rear shock: a spring round a rod, its reservoir.
    p.cyl(0.012, 0.36, (-0.32, 0, 0.6), "Steel", rot=(0, -30, 0), segments=5)
    for i in range(4):
        t = i / 3
        p.torus(0.032, 0.006, (-0.38 + t * 0.12, 0, 0.5 + t * 0.21), "Hazard yellow", rot=(0, -30, 0), segments=5, sides=5)
    # Forks: stanchions in their sliders, the clamps, bars and controls.
    for s in (-1, 1):
        p.tube([(0.72, s * 0.08, 0.34), (0.58, s * 0.08, 0.66)], 0.032, "Gunmetal", segments=5)
        p.tube([(0.6, s * 0.08, 0.62), (0.42, s * 0.08, 1.02)], 0.02, "Steel", segments=5)
    for z in (0.88, 1.0):
        p.span((0.42, -0.12, z - 0.025), (0.5, 0.12, z + 0.025), "Hull dark")
    p.tube([(0.36, -0.38, 1.12), (0.42, -0.3, 1.07), (0.42, 0.3, 1.07), (0.36, 0.38, 1.12)], 0.014, "Hull dark", segments=5)
    for s in (-1, 1):
        p.cyl(0.022, 0.12, (0.36, s * 0.37, 1.12), "Rubber", rot=(90, 0, 0), segments=5)
        p.tube([(0.4, s * 0.28, 1.1), (0.46, s * 0.36, 1.08)], 0.006, "Steel", segments=5)
        p.cable((0.4, s * 0.25, 1.08), (0.3, s * 0.05, 0.85), 0.006, "Polymer", sag=0.05, segments=5, steps=3)
    # Engine: crankcase, a finned barrel and head, the carburettor.
    p.lathe([(0.0, -0.13), (0.13, -0.13), (0.16, -0.08), (0.16, 0.08), (0.13, 0.13), (0.0, 0.13)], (-0.02, 0, 0.42), "Hull alloy",
            rot=(90, 0, 0), segments=8)
    for i in range(7):
        z = 0.56 + i * 0.035
        p.span((0.0, -0.11, z), (0.22, 0.11, z + 0.016), "Gunmetal")
    p.span((0.03, -0.07, 0.55), (0.19, 0.07, 0.81), "Gunmetal")
    p.cyl(0.04, 0.12, (-0.08, -0.04, 0.7), "Steel", rot=(0, 90, 0), segments=5)
    # The exhaust: header, a heat shield, the silencer high at the back.
    p.tube([(0.2, -0.06, 0.66), (0.32, -0.12, 0.45), (0.1, -0.16, 0.3), (-0.2, -0.17, 0.34), (-0.42, -0.17, 0.38)], 0.024, "Rust", segments=5)
    p.lathe([(0.0, 0.0), (0.045, 0.0), (0.055, 0.04), (0.055, 0.3), (0.04, 0.34), (0.0, 0.34)], (-0.42, -0.17, 0.38), "Steel",
            rot=(0, -85, 0), segments=6)
    # The chain over its sprockets.
    p.cyl(0.05, 0.012, (-0.02, 0.1, 0.42), "Gunmetal", rot=(90, 0, 0), segments=6)
    p.cyl(0.1, 0.012, (-0.72, 0.1, 0.34), "Gunmetal", rot=(90, 0, 0), segments=8)
    for z0, z1 in ((0.47, 0.44), (0.37, 0.24)):
        p.tube([(-0.02, 0.1, z0), (-0.72, 0.1, z1)], 0.008, "Hull dark", segments=5)
    # Tank, seat, side panels, fenders, number board and the lamp.
    p.prism([(-0.08, 0.78), (0.38, 0.86), (0.34, 1.0), (-0.02, 0.97)], 0.3, (0, 0, 0), paint)
    p.prism([(0.08, 0.8), (0.18, 0.82), (0.17, 0.95), (0.09, 0.94)], 0.32, (0, 0, 0), r.choice(REPAINTS))
    p.cyl(0.04, 0.03, (0.18, 0, 1.0), "Hull alloy", segments=5)
    p.prism([(-0.82, 0.83), (-0.08, 0.86), (0.0, 0.96), (-0.82, 0.92)], 0.25, (0, 0, 0), "Leather")
    p.prism([(-0.75, 0.6), (-0.3, 0.62), (-0.22, 0.8), (-0.75, 0.8)], 0.27, (0, 0, 0), paint)
    p.prism([(-1.04, 0.6), (-0.7, 0.78), (-0.25, 0.8), (-0.25, 0.77), (-0.7, 0.74), (-1.02, 0.55)], 0.16, (0, 0, 0), paint)
    p.prism([(0.55, 0.74), (0.92, 0.58), (0.94, 0.62), (0.6, 0.8)], 0.15, (0, 0, 0), paint)
    p.prism([(0.42, 0.95), (0.57, 0.9), (0.55, 1.2), (0.44, 1.2)], 0.18, (0, 0, 0), "Bleached")
    p.lathe([(0.0, 0.0), (0.07, 0.0), (0.075, 0.05), (0.06, 0.08), (0.0, 0.08)], (0.56, 0, 1.06), "Hull dark", rot=(0, 90, 0), segments=6)
    p.cyl(0.055, 0.012, (0.644, 0, 1.06), "Sodium lamp", rot=(0, 90, 0), segments=6)
    p.cyl(0.035, 0.03, (-1.04, 0, 0.66), "Fabric red", rot=(0, 90, 0), segments=5)
    with p.painted():
        p.stencil("9", (-0.5, -0.141, 0.72), 0.14, "Hull dark", facing="-Y")
        p.stencil("9", (-0.5, 0.141, 0.72), 0.14, "Hull dark", facing="+Y")
    # Panniers either side of the rear wheel, strapped on.
    for s in (-1, 1):
        y0, y1 = sorted((s * 0.15, s * 0.33))
        p.span((-0.95, y0, 0.55), (-0.45, y1, 0.86), "Canvas")
        p.span((-0.96, min(y0, y1) - 0.006, 0.76), (-0.44, max(y0, y1) + 0.006, 0.88), "Fabric olive")
        p.span((-0.76, min(y0, y1) - 0.012, 0.53), (-0.72, max(y0, y1) + 0.012, 0.9), "Leather")
    p.collider((2.1, 0.6, 1.0), (0, 0, 0.6))
    p.turn(-1)  # nose along -Y
    return p


def trike(style: Style) -> Piece:
    """A three-wheeled Fringe runabout: a single front wheel on a sprung
    fork, a tub over the back axle, a bench seat under a roll bar, and a
    rack of water cans and filters roped down behind."""
    p = Piece("trike", "vehicle", "2.7 × 1.7 × 1.75 m trike, front at -Y")
    p.budget = 30000
    r = rng(style, p.name)
    paint = r.choice(["Bleached", "Repaint green", "Repaint cream", "Crew grey"])
    _spoked(p, 1.0, 0.36, 0.36, 0.2, knobs=24)
    for y, s in ((-0.62, -1), (0.62, 1)):
        _wheel(p, -0.85, y, 0.42, 0.42, 0.3, s, lugs=16)
    p.cyl(0.05, 1.3, (-0.85, 0, 0.42), "Hull dark", rot=(90, 0, 0), segments=5)
    p.lathe([(0.0, -0.14), (0.12, -0.13), (0.16, 0.0), (0.12, 0.13), (0.0, 0.14)], (-0.85, 0, 0.42), "Gunmetal", rot=(90, 0, 0), segments=6)
    # The tub over the back axle, its nose, the frame forward to the fork.
    p.span((-1.3, -0.5, 0.48), (0.2, 0.5, 0.95), paint)
    p.prism([(0.2, 0.48), (0.86, 0.66), (0.86, 0.85), (0.2, 0.95)], 0.5, (0, 0, 0), paint)
    p.tube([(-0.2, 0, 0.5), (0.5, 0, 0.55), (0.8, 0, 0.95)], 0.035, "Hull dark", segments=5)
    for s in (-1, 1):
        y0, y1 = sorted((s * 0.45, s * 0.8))
        p.span((-1.25, y0, 0.88), (-0.45, y1, 0.93), "Hull dark")
        p.span((-1.25, y0, 0.78), (-1.22, y1, 0.88), "Hull dark")
        p.tube([(1.0, s * 0.11, 0.36), (0.74, s * 0.11, 1.05)], 0.03, "Steel", segments=5)
        p.tube([(0.98, s * 0.11, 0.4), (0.86, s * 0.11, 0.72)], 0.042, "Gunmetal", segments=5)
    p.tube([(0.62, -0.4, 1.2), (0.72, -0.3, 1.12), (0.72, 0.3, 1.12), (0.62, 0.4, 1.2)], 0.018, "Hull dark", segments=5)
    for s in (-1, 1):
        p.cyl(0.024, 0.12, (0.62, s * 0.41, 1.2), "Rubber", rot=(90, 0, 0), segments=5)
    p.lathe([(0.0, 0.0), (0.08, 0.0), (0.085, 0.05), (0.07, 0.09), (0.0, 0.09)], (0.86, 0, 0.8), "Hull dark", rot=(0, 90, 0), segments=6)
    p.cyl(0.065, 0.012, (0.955, 0, 0.8), "Sodium lamp", rot=(0, 90, 0), segments=6)
    # The bench seat and the roll bar over it.
    p.span((-0.35, -0.42, 0.95), (0.15, 0.42, 1.08), "Leather")
    p.span((-0.42, -0.42, 1.08), (-0.3, 0.42, 1.45), "Leather")
    p.tube([(-0.47, -0.5, 0.95), (-0.47, -0.5, 1.72), (-0.47, 0.5, 1.72), (-0.47, 0.5, 0.95)], 0.035, "Hull dark", segments=5)
    p.tube([(-0.47, -0.5, 1.6), (-1.25, -0.48, 0.96)], 0.025, "Hull dark", segments=5)
    p.tube([(-0.47, 0.5, 1.6), (-1.25, 0.48, 0.96)], 0.025, "Hull dark", segments=5)
    # The rack: water cans and a crate of filters, roped down.
    p.span((-1.3, -0.5, 0.95), (-0.5, 0.5, 1.0), "Grating")
    for y in (-0.3, 0.05):
        p.source("metal_jerrycan_green", at=(-1.12, y, 1.0), rot=(0, 0, 90), height=0.46)
    p.source("plastic_crate_02", at=(-0.72, 0.0, 1.0), rot=(0, 0, 90), length=0.4)
    for y in (-0.08, 0.08):
        for x in (-0.82, -0.62):
            p.cyl(0.045, 0.12, (x, y, 1.33), "Fabric sand", segments=5)
    p.cable((-1.3, -0.52, 1.45), (-0.5, -0.52, 1.3), 0.01, "Canvas", sag=0.03, segments=5)
    p.cable((-1.3, 0.52, 1.45), (-0.5, 0.52, 1.3), 0.01, "Canvas", sag=0.03, segments=5)
    p.collider((2.3, 1.2, 1.1), (-0.15, 0, 0.6 + 0.4))
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


PIECES = [hauler, hauler_tanker, bike, trike, drifter_patience, receivership_shuttle, covered_car]
