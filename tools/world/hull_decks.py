"""The rest of the Hull, beyond the market street (docs/settlement.md): the
lower decks, where the Crew keeps the air plant running; the Stacks, the
upper decks cut into cramped rooms where every new arrival starts; the
warehouses; and the market's food and parts stalls.

The Crew's look is utility orange and grey with numbers stencilled on
everything (docs/look-and-feel.md); the Stacks are cloth, cots and whatever
could be carried up a ladder. Real things (the compressor, the stool, the
kettle, the goods on the stalls) are Poly Haven models; the rest is made
here, to tools/world/GUIDE.md's bar. Like the kit, every piece stands on its
origin with its front looking along -Y, and wall-mounted pieces have their
backs at y = 0."""
import math

import kit
from kit import DECK, FABRICS, GRID, PALETTE, REPAINTS, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "Hull decks"

H = GRID / 2
# Walls side by side overlap their colliders this much (as hull_kit's do),
# so nothing slips through the seam between two.
SEAM = 0.02
# The big machines of the air plant: placed a few times, so they get detail.
MACHINE = 30000
# The stalls and racks, heaped with goods (Poly Haven's, among others): kept
# under this before the finish, so nothing has to be decimated.
STALL = 45000

PALETTE.setdefault("Steam", (0.62, 0.62, 0.60))
PALETTE.setdefault("Tomato", (0.55, 0.06, 0.03))
# The same as items.py's, for produce on the stalls when it isn't loaded.
PALETTE.setdefault("Veg red", (0.55, 0.04, 0.02))
PALETTE.setdefault("Veg orange", (0.75, 0.25, 0.02))
PALETTE.setdefault("Veg green", (0.10, 0.32, 0.04))


# Helpers ------------------------------------------------------------------

def _bolts(p: Piece, points, r=0.012, h=0.012, mat="Hull dark", facing="-Y") -> None:
    """Bolt heads on a face: points are where they sit on it; facing is the
    way the face looks."""
    rot, d = {"-Y": ((90, 0, 0), (0, -1, 0)), "+Y": ((90, 0, 0), (0, 1, 0)),
              "-X": ((0, 90, 0), (-1, 0, 0)), "+X": ((0, 90, 0), (1, 0, 0)),
              "+Z": ((0, 0, 0), (0, 0, 1))}[facing]
    for x, y, z in points:
        p.cyl(r, h, (x + d[0] * h / 2, y + d[1] * h / 2, z + d[2] * h / 2), mat, rot=rot, segments=4)


def _ring(p: Piece, r_in, r_out, depth, at, mat: str, rot=(0, 0, 0), segments=24) -> None:
    """A thick ring (a duct's shroud, a flange): r_in to r_out, depth along
    its axis (Z before rot), centred on at."""
    from mathutils import Euler, Matrix, Vector
    m = Matrix.Translation(at) @ Euler([math.radians(a) for a in rot]).to_matrix().to_4x4()
    n = kit._n(segments)
    rings = []
    for r, z in ((r_in, -depth / 2), (r_out, -depth / 2), (r_out, depth / 2), (r_in, depth / 2)):
        rings.append([p.bm.verts.new(m @ Vector((math.cos(a) * r, math.sin(a) * r, z)))
                      for a in (math.tau * i / n for i in range(n))])
    faces = []
    for k in range(4):
        a, b = rings[k], rings[(k + 1) % 4]
        for i in range(n):
            faces.append(p.bm.faces.new((a[i], a[(i + 1) % n], b[(i + 1) % n], b[i])))
    p._faces(faces, mat, smooth=False)


def _hazard(p: Piece, x0, x1, z0, z1, y, width=0.1, stripe="Hull dark", base=None) -> None:
    """Painted diagonal hazard stripes on a face looking along -Y at y, from
    x0 to x1 and z0 to z1 (base, if given, is painted under them first)."""
    with p.painted():
        if base:
            p.span((x0, y - 0.005, z0), (x1, y - 0.004, z1), base)
        h = z1 - z0
        x = x0 - h
        while x < x1:
            poly = _clip([(x, z0), (x + width, z0), (x + width + h, z1), (x + h, z1)], x0, x1, z0, z1)
            if len(poly) >= 3:
                p.prism(poly, 0.002, (0, y - 0.01, 0), stripe)
            x += 2 * width


def _clip(poly, x0, x1, z0, z1):
    """A convex polygon cut to a rectangle (Sutherland–Hodgman)."""
    def cut(points, inside, cross):
        out = []
        for i, a in enumerate(points):
            b = points[(i + 1) % len(points)]
            if inside(a):
                out.append(a)
                if not inside(b):
                    out.append(cross(a, b))
            elif inside(b):
                out.append(cross(a, b))
        return out

    def at_x(x):
        return lambda a, b: (x, a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0]))

    def at_z(z):
        return lambda a, b: (a[0] + (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]), z)

    for inside, cross in ((lambda p: p[0] >= x0, at_x(x0)), (lambda p: p[0] <= x1, at_x(x1)),
                          (lambda p: p[1] >= z0, at_z(z0)), (lambda p: p[1] <= z1, at_z(z1))):
        poly = cut(poly, inside, cross)
        if not poly:
            break
    return poly


def _gauge(p: Piece, at, r=0.06, facing="-Y", face="Paper") -> None:
    """A round dial on a face: a bezel, its face and a needle painted on."""
    x, y, z = at
    s = -1 if facing == "-Y" else 1
    p.cyl(r, 0.04, (x, y + s * 0.02, z), "Hull dark", rot=(90, 0, 0), segments=12)
    p.cyl(r * 0.82, 0.01, (x, y + s * 0.042, z), face, rot=(90, 0, 0), segments=12)
    with p.painted():
        p.box((r * 0.75, 0.002, r * 0.08), (x + r * 0.2, y + s * 0.053, z + r * 0.15), "Fabric red", rot=(0, -35, 0))


def _switches(p: Piece, x0, x1, z, y, n, mat="Hull alloy") -> None:
    """A row of toggle switches on a panel looking along -Y at y."""
    for i in range(n):
        x = x0 + (x1 - x0) * (i + 0.5) / n
        p.box((0.03, 0.02, 0.04), (x, y - 0.01, z), "Hull dark")
        p.box((0.008, 0.04, 0.008), (x, y - 0.035, z + 0.008), mat, rot=(25 if i % 3 else -25, 0, 0))


def _wall(p: Piece, style: Style, x_open=None, z_open=0.0) -> None:
    """A 2 m hull wall a deck high, 0.3 m thick over its frame, plated on
    both faces in patched repaint, with a doorway from x_open[0] to x_open[1]
    and up to z_open if given. Its collider is the caller's."""
    r = rng(style, p.name + "wall")
    cuts = [(-H, H)] if not x_open else [(-H, x_open[0]), (x_open[1], H)]
    for x0, x1 in cuts:
        p.span((x0, -0.1, 0), (x1, 0.1, DECK), "Hull alloy")
    if x_open:
        p.span((x_open[0], -0.1, z_open), (x_open[1], 0.1, DECK), "Hull alloy")
    for side in (-1, 1):
        y0, y1 = sorted((side * 0.1, side * 0.15))
        # Ribs at the ends, a kick strip and a stringer under the deck above.
        for x in (-H, H - 0.12):
            p.span((x, y0, 0), (x + 0.12, y1, DECK), "Hull dark")
            _bolts(p, [(x + 0.06, side * 0.15, z) for z in (0.4, 1.2, 2.0, 2.8)], facing="+Y" if side > 0 else "-Y")
        for z0, z1 in ((0, 0.18), (DECK - 0.22, DECK - 0.1)):
            for x0, x1 in cuts if z0 == 0 else [(-H, H)]:
                p.span((max(x0, -H + 0.12), y0, z0), (min(x1, H - 0.12), y1, z1), "Hull dark")
        # Plates, overlapping like patches, each riveted at its corners.
        cols, rows = 2, 3
        w, h = (2 * H - 0.24) / cols, (DECK - 0.4) / rows
        for c in range(cols):
            for row in range(rows):
                a = (-H + 0.12 + c * w + 0.02, 0.18 + row * h + 0.02)
                b = (-H + 0.12 + (c + 1) * w - 0.02, 0.18 + (row + 1) * h - 0.02)
                if x_open and a[0] < x_open[1] + 0.06 and b[0] > x_open[0] - 0.06 and a[1] < z_open + 0.12:
                    continue
                mat = r.choice(REPAINTS) if r.random() < style.repaint else "Hull alloy"
                lift = 0.015 + 0.012 * (row % 2)
                y = side * (0.1 + lift)
                p.span((a[0], min(side * 0.1, y), a[1]), (b[0], max(side * 0.1, y), b[1]), mat)
                rivets = [(x, y, z) for x in (a[0] + 0.05, b[0] - 0.05) for z in (a[1] + 0.05, b[1] - 0.05)]
                _bolts(p, rivets, r=0.009, h=0.006, mat=mat, facing="+Y" if side > 0 else "-Y")


# The lower decks: the Crew's air plant ------------------------------------

def air_scrubber(style: Style) -> Piece:
    """A scrubber bank: the air plant draws the Hull's air through racks of
    filter cartridges, the same cartridges the Fringe uses for money. A
    riveted cabinet on a plinth, louvred down its sides, a duct up into the
    deck above and the Crew's numbers on everything."""
    p = Piece("air_scrubber", "prop", "2 × 1 × 3 m filter bank", budget=MACHINE)
    w, d, h = 1.0, 0.5, 2.7
    p.span((-w, -d + 0.05, 0), (w, d, 0.15), "Hull dark")
    # The cabinet is shallow behind an open bay, where the racks are.
    p.span((-w + 0.04, -0.08, 0.15), (w - 0.04, d - 0.02, h), "Crew grey")
    for x in (-w + 0.04, w - 0.08):
        p.span((x, -d + 0.1, 0.16), (x + 0.04, -0.08, h - 0.01), "Crew grey")
    p.collider((2 * w, 2 * d, 3.0), (0, 0, 1.5))
    # The open front's frame, and its shelves of cartridges.
    for x in (-w, w - 0.1):
        p.span((x, -d, 0.15), (x + 0.1, -d + 0.14, h), "Crew orange")
    p.span((-w, -d, h - 0.14), (w, -d + 0.14, h), "Crew orange")
    _bolts(p, [(x, -d, z) for x in (-w + 0.05, w - 0.05) for z in (0.6, 1.4, 2.2)])
    rows, cols = 4, 5
    for row in range(rows):
        z = 0.32 + row * 0.56
        p.span((-w + 0.1, -d + 0.13, z - 0.05), (w - 0.1, -0.08, z - 0.02), "Hull alloy")
        p.span((-w + 0.1, -d + 0.1, z - 0.07), (w - 0.1, -d + 0.13, z - 0.02), "Crew orange")
        for c in range(cols):
            x = -w + 0.26 + c * (2 * w - 0.52) / (cols - 1)
            p.cyl(0.11, 0.26, (x, -d + 0.3, z + 0.11), "Fabric sand", rot=(90, 0, 0), segments=8)
            p.cyl(0.116, 0.045, (x, -d + 0.15, z + 0.11), "Crew orange", rot=(90, 0, 0), segments=8)
    # Louvres down both sides.
    for sx in (-1, 1):
        for i in range(6):
            z = 0.5 + i * 0.32
            p.box((0.03, 0.6, 0.1), (sx * (w - 0.025), 0.05, z), "Hull dark", rot=(0, sx * 40, 0))
    # The duct up into the deck above, its flange, clamps and a gauge.
    p.span((-0.6, -0.3, h), (0.6, 0.4, 3.0), "Hull alloy")
    p.span((-0.66, -0.36, h + 0.1), (0.66, 0.46, h + 0.16), "Hull dark")
    _bolts(p, [(x, -0.36, h + 0.13) for x in (-0.5, -0.25, 0, 0.25, 0.5)], r=0.01)
    p.cable((w - 0.2, -0.32, h + 0.05), (w - 0.05, -0.55, 0.05), 0.025, "Rubber", sag=0.1, steps=4, segments=5)
    _gauge(p, (w - 0.25, -d, h + 0.2), 0.07)
    p.cyl(0.05, 0.03, (-w + 0.25, -d - 0.015, h + 0.2), "Status blue", rot=(90, 0, 0), segments=10)
    p.stencil("3", (-0.45, -0.31, h + 0.15), 0.12, "Stencil white")
    p.stencil("14", (0, -d + 0.045, 0.075), 0.08, "Stencil white")
    _hazard(p, -w + 0.1, w - 0.1, 0.02, 0.13, -d + 0.05)
    return p


def air_fan(style: Style) -> Piece:
    """One of the air plant's big ducted fans, the hum you hear all over the
    Hull: an orange shroud with bolted flanges, six pitched blades behind a
    wire guard, the motor at the back, on a braced stand."""
    p = Piece("air_fan", "prop", "3 m ducted fan on a stand", budget=MACHINE)
    c, r, depth = 1.6, 1.2, 0.9
    _ring(p, r, r + 0.08, depth, (0, 0, c), "Crew orange", rot=(90, 0, 0), segments=18)
    for y in (-depth / 2, depth / 2):
        _ring(p, r - 0.02, r + 0.16, 0.06, (0, y, c), "Hull dark", rot=(90, 0, 0), segments=20)
        for i in range(8):
            a = math.tau * (i + 0.5) / 8
            p.cyl(0.014, 0.08, ((r + 0.12) * math.cos(a), y, c + (r + 0.12) * math.sin(a)), "Hull alloy", rot=(90, 0, 0), segments=4)
    # The guard: rings and spokes.
    for rr in (0.35, 0.7, 1.05):
        p.torus(rr, 0.012, (0, -depth / 2 - 0.02, c), "Hull dark", rot=(90, 0, 0), segments=16, sides=2)
    for i in range(8):
        a = math.tau * i / 8
        p.box((r * 2 - 0.05, 0.02, 0.02), (0, -depth / 2 - 0.02, c), "Hull dark", rot=(0, math.degrees(a), 0))
    # Hub, blades and the motor behind.
    p.lathe([(0, -0.35), (0.12, -0.3), (0.2, -0.15), (0.22, 0.2), (0.0, 0.2)], (0, 0, c), "Hull alloy", rot=(-90, 0, 0), segments=16)
    for i in range(6):
        a = math.tau * i / 6
        p.box((0.32, 0.03, r - 0.25), (math.cos(a) * (r / 2 + 0.05), 0.0, c + math.sin(a) * (r / 2 + 0.05)), "Crew grey",
              rot=(25, -math.degrees(a) + 90, 0))
    p.cyl(0.3, 0.5, (0, depth / 2 + 0.2, c), "Crew grey", rot=(90, 0, 0), segments=16)
    for k in range(3):
        p.cyl(0.33, 0.02, (0, depth / 2 + 0.07 + k * 0.12, c), "Hull dark", rot=(90, 0, 0), segments=12)
    for a in (0, 120, 240):
        t = math.radians(a)
        p.box((0.05, depth / 2 + 0.05, 0.05), (math.cos(t) * 0.7, depth / 4 + 0.1, c + math.sin(t) * 0.7), "Hull dark",
              rot=(0, -a, 0))
    p.cable((0.2, depth / 2 + 0.45, c - 0.15), (0.6, depth / 2 + 0.2, 0.03), 0.03, "Rubber", sag=0.15, steps=4, segments=5)
    # The stand: two braced A-frames on a skid.
    # Legs outside the shroud, up to its flanges' sides, braced.
    for x in (-1.36, 1.36):
        for y in (-0.3, 0.3):
            p.span((x - 0.05, y - 0.05, 0.12), (x + 0.05, y + 0.05, c), "Hull dark")
        p.span((x - 0.03, -0.3, 0.6), (x + 0.03, 0.3, 0.66), "Hull dark")
        p.box((0.05, 0.05, 0.85), (x, 0, 0.95), "Hull dark", rot=(35, 0, 0))
        p.span((x * 0.9 - 0.06, -0.08, c - 0.06), (x + 0.05, 0.08, c + 0.06), "Hull dark")
    p.span((-1.35, -0.6, 0), (1.35, 0.6, 0.12), "Crew grey")
    _hazard(p, -1.35, 1.35, 0.01, 0.11, -0.6)
    p.stencil("7", (0, -0.606, 0.065), 0.07, "Stencil white")
    p.collider((2.7, depth + 0.1, 2.9), (0, 0, 1.45))
    return p


def pump_unit(style: Style) -> Piece:
    """A pump on its skid, moving water round the lower decks: an old
    military compressor the Crew rebuilt and painted their own, its outlet
    plumbed down into the deck."""
    p = Sourced("pump_unit", "prop", "1.6 m pump on a skid, 1.15 m high: mantle", asset="old_military_compressor",
                res="2k", length=1.6, rot=(0, 0, 90), recolour="Crew orange", at=(0, 0, 0.08))
    p.span((-0.85, -0.38, 0), (0.85, 0.38, 0.08), "Hull dark")
    _bolts(p, [(x, y, 0.08) for x in (-0.75, 0.75) for y in (-0.3, 0.3)], facing="+Z")
    p.cable((0.7, -0.2, 0.55), (0.95, -0.55, 0.0), 0.05, "Rubber", sag=0.05, steps=4, segments=5)
    p.cable((-0.75, 0.15, 0.4), (-1.0, 0.45, 0.0), 0.035, "Hull dark", sag=0.04, steps=4, segments=5)
    _hazard(p, -0.85, 0.85, 0.005, 0.075, -0.38)
    with p.painted():
        p.stencil("21", (-0.5, -0.386, 0.04), 0.05, "Stencil white")
    p.collider((1.6, 0.6, 1.1), (0, 0, 0.55))
    return p


def valve_station(style: Style) -> Piece:
    """A manifold on the lower decks: two risers into a header, and the big
    valve that a repair run comes to replace, its wheel facing the walkway."""
    p = Piece("valve_station", "prop", "1.6 m manifold with a valve wheel; free-standing", budget=MACHINE)
    for x in (-0.6, 0.6):
        p.cyl(0.08, 2.58, (x, 0.15, 1.31), "Rust", segments=14)
        for z in (0.12, 1.25, 2.5):
            p.cyl(0.12, 0.05, (x, 0.15, z), "Hull dark", segments=14)
            _bolts(p, [(x + 0.1 * math.cos(a), 0.15 + 0.1 * math.sin(a), z + 0.025) for a in (0, 1.57, 3.14, 4.71)],
                   r=0.008, h=0.01, facing="+Z")
    p.cyl(0.09, 1.4, (0, 0.15, 1.25), "Crew orange", rot=(0, 90, 0), segments=14)
    # The valve: a body, its bonnet and stem, and the wheel.
    p.lathe([(0, -0.18), (0.14, -0.16), (0.16, 0.0), (0.14, 0.16), (0, 0.18)], (0, 0.15, 1.25), "Hull alloy", rot=(0, 90, 0), segments=14)
    p.cyl(0.07, 0.25, (0, -0.02, 1.25), "Hull alloy", rot=(90, 0, 0), segments=12)
    p.cyl(0.02, 0.22, (0, -0.2, 1.25), "Steel", rot=(90, 0, 0), segments=8)
    p.torus(0.22, 0.022, (0, -0.3, 1.25), "Fabric red", rot=(90, 0, 0), segments=24, sides=6)
    for i in range(5):
        a = math.tau * i / 5
        p.box((0.22, 0.02, 0.025), (0.11 * math.cos(a), -0.3, 1.25 + 0.11 * math.sin(a)), "Fabric red", rot=(0, -math.degrees(a), 0))
    p.cyl(0.04, 0.05, (0, -0.3, 1.25), "Hull dark", rot=(90, 0, 0), segments=8)
    # Gauges on stubs, a drip tray, the tag on its chain.
    for x in (-0.6, 0.6):
        p.cyl(0.015, 0.15, (x, 0.0, 1.8), "Steel", rot=(90, 0, 0), segments=6)
        _gauge(p, (x, -0.08, 1.8), 0.07)
    p.span((-0.75, -0.1, 0), (0.75, 0.4, 0.05), "Hull dark")
    p.span((-0.7, -0.05, 0.05), (0.7, 0.35, 0.06), "Grating")
    p.cable((0.05, -0.3, 1.15), (0.05, -0.31, 0.95), 0.004, "Steel", steps=3, segments=5)
    p.box((0.08, 0.004, 0.05), (0.05, -0.31, 0.92), "Paper aged")
    with p.painted():
        p.span((-0.2, 0.055, 1.42), (0.2, 0.057, 1.6), "Crew orange")
    p.collider((1.4, 0.35, 2.6), (0, 0.15, 1.3))
    return p


def junction_box(style: Style) -> Piece:
    """A Crew power box on a wall, back at y = 0: a hinged steel door with a
    latch and a warning plate, conduit glands top and bottom, and the
    conduits running up to the deck above and down to the floor."""
    p = Piece("junction_box", "prop", "0.6 × 0.26 m wall box with conduits; no collider")
    z0, z1 = 1.0, 1.7
    p.span((-0.3, -0.2, z0), (0.3, 0.0, z1), "Crew grey")
    p.span((-0.28, -0.22, z0 + 0.02), (0.28, -0.2, z1 - 0.02), "Crew grey")
    p.span((-0.24, -0.235, z0 + 0.06), (0.24, -0.22, z1 - 0.06), "Hull alloy")
    for z in (z0 + 0.12, z1 - 0.12):
        p.cyl(0.015, 0.06, (-0.29, -0.215, z), "Hull dark", segments=6)
    p.box((0.04, 0.03, 0.12), (0.2, -0.25, (z0 + z1) / 2), "Hull dark")
    p.cyl(0.02, 0.03, (0.2, -0.27, (z0 + z1) / 2 + 0.04), "Brass", rot=(90, 0, 0), segments=6)
    _bolts(p, [(x, -0.235, z) for x in (-0.22, 0.22) for z in (z0 + 0.08, z1 - 0.08)], r=0.008, h=0.005, mat="Hull alloy")
    _hazard(p, -0.15, 0.15, z1 - 0.2, z1 - 0.12, -0.235, width=0.04, base="Hazard yellow")
    for x in (-0.18, -0.06, 0.06, 0.18):
        p.cyl(0.025, 0.06, (x, -0.1, z1 + 0.03), "Hull dark", segments=8)
        p.cyl(0.018, DECK - z1 - 0.06, (x, -0.1, (DECK + z1 + 0.06) / 2), "Hull alloy" if x < 0 else "Crew orange", segments=8)
    for x in (-0.12, 0.0, 0.12):
        p.cyl(0.025, 0.06, (x, -0.1, z0 - 0.03), "Hull dark", segments=8)
        p.cyl(0.018, z0 - 0.06, (x, -0.1, (z0 - 0.06) / 2), "Hull alloy", segments=8)
    for z in (0.4, 2.4, 3.1):
        p.span((-0.24, -0.14, z - 0.02), (0.24, -0.0, z + 0.02), "Hull dark")
    p.cyl(0.035, 0.03, (-0.15, -0.25, z1 - 0.06), "Status blue", rot=(90, 0, 0), segments=8)
    p.stencil("43", (0.0, -0.24, z0 + 0.15), 0.08, "Stencil white")
    return p


def power_conduit(style: Style) -> Piece:
    """2 m of power run along a deck floor: a steel tray with ramped edges,
    four cables in it, strapped down and hazard-striped so you look where
    you put your feet. Low enough to walk over; tiled, so the game draws a
    low stand-in with all this baked onto it."""
    p = Piece("power_conduit", "prop", "2 m floor cable tray, 0.12 m high; walked over")
    w, h = 0.2, 0.11
    p.prism([(-w - 0.08, 0), (w + 0.08, 0), (w, h), (-w, h)], 2 * H, (0, 0, 0), "Crew grey", rot=(0, 0, 90))
    for i, mat in enumerate(("Fabric red", "Hull dark", "Crew orange", "Rubber")):
        y = -0.12 + i * 0.08
        p.cable((-H, y, h + 0.01), (H, y, h + 0.01), 0.025, mat, steps=2, segments=5)
    for x in (-0.7, 0.0, 0.7):
        p.span((x - 0.04, -w - 0.02, h - 0.01), (x + 0.04, w + 0.02, h + 0.045), "Hull alloy")
        _bolts(p, [(x, y, h + 0.045) for y in (-w + 0.02, w - 0.02)], r=0.01, h=0.008, facing="+Z")
    with p.painted():
        for sy in (-1, 1):
            for i in range(10):
                x = -H + 0.1 + i * 0.2
                p.box((0.08, 0.06, 0.004), (x, sy * (w + 0.04), h / 2 + 0.007), "Hazard yellow", rot=(sy * 55, 0, 30))
    with p.lowpoly():
        p.prism([(-w - 0.08, 0), (w + 0.08, 0), (w, h + 0.03), (-w, h + 0.03)], 2 * H, (0, 0, 0), "Crew grey", rot=(0, 0, 90))
    return p


def generator(style: Style) -> Piece:
    """A fuel-cell stack: banks of cells in a braced frame, cooling fins down
    its back, three vent stacks on top, a control panel of gauges and lamps,
    and fat cables sagging to the floor."""
    p = Piece("generator", "prop", "1.2 × 0.9 × 2.4 m fuel-cell stack", budget=MACHINE)
    w, d = 0.6, 0.45
    p.span((-w - 0.02, -d - 0.02, 0), (w + 0.02, d + 0.02, 0.12), "Hull dark")
    for x in (-w, w - 0.06):
        for y in (-d, d - 0.06):
            p.span((x, y, 0.12), (x + 0.06, y + 0.06, 2.1), "Hull dark")
    for z in (0.12, 0.7, 1.4, 2.04):
        p.span((-w, -d, z), (w, -d + 0.06, z + 0.06), "Hull dark")
        p.span((-w, d - 0.06, z), (w, d, z + 0.06), "Hull dark")
    p.span((-w + 0.06, -d + 0.1, 0.18), (w - 0.06, d - 0.06, 2.04), "Crew grey")
    # The cells: rows of plates in orange casings.
    for row in range(2):
        z0 = 0.2 + row * 0.7
        p.span((-w + 0.08, -d + 0.04, z0), (w - 0.08, -d + 0.1, z0 + 0.48), "Crew orange")
        for i in range(9):
            x = -w + 0.14 + i * 0.115
            p.span((x - 0.035, -d + 0.02, z0 + 0.04), (x + 0.035, -d + 0.04, z0 + 0.44), "Hull alloy")
    # Fins down the back.
    for i in range(12):
        x = -w + 0.08 + i * 0.095
        p.span((x - 0.012, d, 0.3), (x + 0.012, d + 0.12, 1.9), "Hull alloy")
    # Vent stacks with rain caps.
    for x in (-0.3, 0.0, 0.3):
        p.cyl(0.06, 0.38, (x, 0.1, 2.31), "Hull alloy", segments=12)
        p.cyl(0.09, 0.03, (x, 0.1, 2.52), "Hull dark", segments=12)
    p.span((-w, -d, 2.1), (w, d, 2.14), "Hull dark")
    # The control panel.
    p.span((-0.45, -d - 0.06, 1.55), (0.45, -d + 0.02, 1.95), "Hull alloy")
    for i, x in enumerate((-0.3, -0.1, 0.1)):
        _gauge(p, (x, -d - 0.06, 1.8), 0.06)
    for i, x in enumerate((0.25, 0.33)):
        p.cyl(0.025, 0.03, (x, -d - 0.075, 1.82), "Status blue" if i else "Sodium lamp", rot=(90, 0, 0), segments=8)
    _switches(p, -0.4, 0.4, 1.63, -d - 0.06, 7)
    for x in (-0.35, 0.35):
        p.cable((x, d + 0.05, 0.4), (x * 1.6, d + 0.5, 0.04), 0.035, "Rubber", sag=0.1, steps=4, segments=5)
    with p.painted():
        p.stencil("9", (0, -d - 0.026, 0.06), 0.07, "Stencil white")
        p.span((0.1, -d - 0.066, 1.9), (0.4, -d - 0.064, 1.94), "Hazard yellow")
    p.collider((1.2, 0.9, 2.1), (0, 0, 1.05))
    return p


def water_tank(style: Style) -> Piece:
    """A riveted steel water tank on a skirt: hoop bands, a ladder up its
    side, a sight glass, the outlet valve and the manhole on its dome. The
    Hull's water, which the Crew controls."""
    p = Piece("water_tank", "prop", "1.8 m across, 2.8 m high", budget=MACHINE)
    r = 0.85
    p.lathe([(0, 0.25), (r - 0.05, 0.25), (r, 0.32), (r, 2.4), (r * 0.8, 2.62), (r * 0.4, 2.74), (0, 2.76)],
            (0, 0, 0), "Repaint teal", segments=24)
    p.lathe([(0, 0.0), (r - 0.04, 0.0), (r - 0.04, 0.3), (0, 0.3)], (0, 0, 0), "Hull dark", segments=24)
    for z in (0.5, 1.0, 1.5, 2.0, 2.35):
        p.torus(r + 0.01, 0.022, (0, 0, z), "Rust", segments=28, sides=6)
    # Sight glass and outlet.
    p.cyl(0.025, 1.6, (r * 0.7, -r * 0.72, 1.3), "Glass", segments=8)
    for z in (0.5, 2.1):
        p.box((0.06, 0.12, 0.05), (r * 0.66, -r * 0.68, z), "Hull dark", rot=(0, 0, -45))
    p.cyl(0.06, 0.4, (0, -r - 0.15, 0.45), "Hull alloy", rot=(90, 0, 0), segments=12)
    p.lathe([(0, -0.1), (0.1, -0.09), (0.11, 0.09), (0, 0.1)], (0, -r - 0.3, 0.45), "Hull alloy", rot=(90, 0, 0), segments=12)
    p.torus(0.1, 0.012, (0, -r - 0.32, 0.6), "Fabric red", segments=16, sides=4)
    p.cyl(0.012, 0.15, (0, -r - 0.32, 0.53), "Steel", segments=6)
    # The ladder up the side, and the manhole.
    for x in (-0.2, 0.2):
        p.box((0.04, 0.04, 2.5), (r + 0.14, x, 1.45), "Hull dark")
    for k in range(8):
        p.cyl(0.015, 0.4, (r + 0.14, 0, 0.4 + k * 0.3), "Hull alloy", rot=(90, 0, 0), segments=6)
    p.cyl(0.22, 0.08, (0.15, 0.1, 2.72), "Hull dark", segments=16)
    p.cyl(0.18, 0.04, (0.15, 0.1, 2.78), "Repaint teal", segments=16)
    p.stencil("2", (0, -r - 0.005, 1.7), 0.35, "Stencil white")
    p.collider((1.7, 1.7, 2.75), (0, 0, 1.375))
    return p


def control_console(style: Style) -> Piece:
    """The Crew's desk in the air plant: a sloped panel of gauges, switches
    and two screens, a backboard of dials and lamps, a multimeter left on
    it, and the cable loom hanging off the back."""
    p = Piece("control_console", "prop", "1.8 × 0.8 m desk, 1.6 m board behind", budget=MACHINE)
    w = 0.9
    p.span((-w, -0.38, 0), (w, 0.2, 0.08), "Hull dark")
    p.span((-w + 0.02, -0.36, 0.08), (w - 0.02, 0.18, 0.78), "Crew grey")
    p.span((-w + 0.1, -0.375, 0.15), (-0.05, -0.36, 0.7), "Crew grey")
    p.span((0.05, -0.375, 0.15), (w - 0.1, -0.36, 0.7), "Crew grey")
    for x in (-0.47, 0.47):
        p.box((0.12, 0.03, 0.03), (x, -0.39, 0.62), "Hull alloy")
    # The sloped panel.
    p.box((2 * w, 0.5, 0.08), (0, -0.2, 0.85), "Crew grey", rot=(-20, 0, 0))
    p.box((2 * w - 0.1, 0.42, 0.02), (0, -0.21, 0.895), "Hull alloy", rot=(-20, 0, 0))
    for i, x in enumerate((-0.65, -0.4)):
        p.box((0.2, 0.15, 0.02), (x, -0.24, 0.91), "Screen", rot=(-20, 0, 0))
    for x in (-0.1, 0.12, 0.34, 0.56):
        p.cyl(0.05, 0.03, (x, -0.22, 0.915), "Hull dark", rot=(-20, 0, 0), segments=10)
        p.cyl(0.04, 0.01, (x, -0.226, 0.932), "Paper", rot=(-20, 0, 0), segments=10)
    for i in range(10):
        x = -0.75 + i * 0.16
        p.box((0.03, 0.03, 0.05), (x, -0.36, 0.84), "Hull dark", rot=(-20, 0, 0))
        p.box((0.008, 0.008, 0.05), (x, -0.37, 0.88), "Hull alloy", rot=(5, 0, 0))
    # The backboard: dials, lamps, a hazard-striped breaker.
    p.span((-w, 0.1, 0.78), (w, 0.22, 1.6), "Crew grey")
    p.span((-w, 0.08, 1.52), (w, 0.24, 1.6), "Crew orange")
    for x in (-0.6, -0.3, 0.0, 0.3):
        _gauge(p, (x, 0.1, 1.25), 0.09)
    for i in range(6):
        p.cyl(0.025, 0.03, (-0.65 + i * 0.12, 0.085, 1.45), "Status blue" if i % 2 else "Sodium lamp", rot=(90, 0, 0), segments=8)
    p.span((0.55, 0.06, 1.05), (0.8, 0.1, 1.45), "Hull dark")
    p.box((0.05, 0.06, 0.15), (0.675, 0.04, 1.3), "Fabric red", rot=(30, 0, 0))
    _hazard(p, 0.55, 0.8, 1.05, 1.12, 0.06, width=0.04, base="Hazard yellow")
    for x in (-0.6, -0.2, 0.2, 0.6):
        p.cable((x, 0.22, 1.0), (x + 0.15, 0.55, 0.03), 0.02, "Rubber", sag=0.12, steps=4, segments=5)
    p.source("retro_multimeter", at=(0.65, -0.05, 0.95), height=0.2, rot=(0, 0, 200))
    with p.painted():
        p.stencil("1", (-0.8, 0.094, 1.4), 0.08, "Stencil white")
        p.stencil("12", (-0.47, -0.381, 0.45), 0.1, "Stencil white")
    p.collider((2 * w, 0.6, 0.95), (0, -0.1, 0.475))
    p.collider((2 * w, 0.2, 1.6), (0, 0.3, 0.8))
    return p


def hazard_barrier(style: Style) -> Piece:
    """A portable barrier: two striped planks on folding A-frames, rubber
    feet and a reflector each end, set across a walkway the Crew's working
    on. 0.95 m high and 0.4 m deep: vaultable."""
    p = Piece("hazard_barrier", "prop", "1.6 m barrier, 0.95 m high: vaultable")
    w, top = 0.8, 0.95
    for x in (-w + 0.1, w - 0.1):
        for s in (-1, 1):
            p.box((0.05, 0.04, top + 0.05), (x, s * 0.14, top / 2), "Hull dark", rot=(s * -14, 0, 0))
            p.box((0.09, 0.09, 0.03), (x, s * 0.25, 0.015), "Rubber")
        p.cyl(0.015, 0.36, (x, 0, 0.35), "Hull alloy", rot=(90, 0, 0), segments=6)
        p.cyl(0.03, 0.06, (x, 0, top - 0.02), "Hull alloy", rot=(90, 0, 0), segments=8)
    for z in (0.55, top - 0.1):
        p.span((-w, -0.04, z - 0.07), (w, 0.04, z + 0.07), "Bleached")
        for s in (-1, 1):
            with p.painted():
                hz = z
                h = 0.14
                x = -w - h
                while x < w:
                    poly = _clip([(x, hz - 0.07), (x + 0.1, hz - 0.07), (x + 0.1 + h, hz + 0.07), (x + h, hz + 0.07)], -w, w, hz - 0.07, hz + 0.07)
                    if len(poly) >= 3:
                        p.prism(poly, 0.002, (0, s * 0.047, 0), "Fabric red")
                    x += 0.2
    for x in (-w + 0.02, w - 0.02):
        p.cyl(0.03, 0.01, (x, -0.058, top - 0.1), "Sodium lamp", rot=(90, 0, 0), segments=8)
    p.collider((2 * w, 0.4, top), (0, 0, top / 2))
    return p


# The Stacks: the rooms upstairs --------------------------------------------

def _blanket(r) -> str:
    return r.choice(["Fabric olive", "Fabric indigo", "Fabric red", "Fabric teal", "Canvas"])


def bunk_bed(style: Style) -> Piece:
    """A two-tier bunk out of steel tube, welded at the joints: slatted
    bases, thin mattresses, rumpled blankets, a pillow each, a ladder at the
    end and a bag hung off the post. Mantle onto the top."""
    p = Piece("bunk_bed", "prop", "2 × 0.9 × 1.7 m bunk; mantle onto it")
    r = rng(style, p.name)
    L, w = 1.0, 0.45
    t = 0.022  # half the square tube's width
    for x in (-L, L):
        for y in (-w, w):
            p.span((x - t, y - t, 0.005), (x + t, y + t, 1.7), "Hull alloy")
            p.span((x - t - 0.008, y - t - 0.008, 0), (x + t + 0.008, y + t + 0.008, 0.02), "Rubber")
    for z in (0.3, 1.15):
        for y in (-w, w):
            p.span((-L + t, y - t * 0.9, z - t * 0.9), (L - t, y + t * 0.9, z + t * 0.9), "Hull alloy")
        for x in (-L, L):
            p.span((x - t * 0.9, -w + t, z - t * 0.9), (x + t * 0.9, w - t, z + t * 0.9), "Hull alloy")
        for i in range(5):
            x = -L + 0.18 + i * 0.41
            p.span((x - 0.04, -w + t, z - 0.015), (x + 0.04, w - t, z + 0.005), "Wood")
        # Mattress, blanket and pillow.
        p.cloth([(-L + 0.03, -w + 0.03, z + 0.11), (L - 0.03, -w + 0.03, z + 0.11), (L - 0.03, w - 0.03, z + 0.11),
                 (-L + 0.03, w - 0.03, z + 0.11)], "Repaint cream", sag=0.0, ripple=0.012, thickness=0.1, cell=0.4, seed=int(z * 10))
        mat = _blanket(r)
        p.cloth([(-L + 0.45, -w + 0.01, z + 0.125), (L - 0.04, -w + 0.01, z + 0.125), (L - 0.04, w - 0.01, z + 0.13),
                 (-L + 0.45, w - 0.01, z + 0.13)], mat, sag=-0.03, ripple=0.012, thickness=0.012, cell=0.07,
                seed=int(z * 7) + 3, droop=(0.3, 0, 1, 0))
        # Its edge hanging over the side.
        p.cloth([(-L + 0.45, -w + 0.01, z + 0.125), (L - 0.04, -w + 0.01, z + 0.125), (L - 0.04, -w - 0.02, z - 0.02),
                 (-L + 0.45, -w - 0.02, z - 0.02)], mat, sag=0.0, ripple=0.0, thickness=0.012, cell=0.2)
        p.sphere(0.2, (-L + 0.22, 0.0, z + 0.17), "Charter white", scale=(0.7, 1.15, 0.35), segments=7, rings=4)
    # The ladder at the end, the guard rail up top.
    for y in (-0.2, 0.2):
        p.span((L + 0.045, y - 0.015, 0.005), (L + 0.075, y + 0.015, 1.4), "Hull alloy")
    for k in range(4):
        p.span((L + 0.05, -0.185, 0.29 + k * 0.28), (L + 0.07, 0.185, 0.31 + k * 0.28), "Hull alloy")
    p.span((-0.85, -w - 0.015, 1.43), (0.45, -w + 0.015, 1.47), "Hull alloy")
    for x in (-0.85, 0.45):
        p.span((x - 0.015, -w - 0.015, 1.17), (x + 0.015, -w + 0.015, 1.43), "Hull alloy")
    # A bag on the post, a strap round it.
    p.sphere(0.16, (-L - 0.12, -w, 1.2), "Repaint green", scale=(0.8, 0.9, 1.3), segments=7, rings=5)
    p.cable((-L, -w, 1.55), (-L - 0.12, -w, 1.38), 0.01, "Leather", sag=0.0, steps=3, segments=5)
    p.collider((2 * L + 0.05, 2 * w + 0.05, 1.7), (0, 0, 0.85))
    return p


def cot(style: Style) -> Piece:
    """A folding camp cot: canvas slung between two poles on X-legs, a
    rolled pillow and a folded blanket. 0.45 m high: vaultable."""
    p = Piece("cot", "prop", "1.9 × 0.7 × 0.45 m: vaultable")
    r = rng(style, p.name)
    L, w, z = 0.95, 0.33, 0.42
    for y in (-w, w):
        p.cyl(0.02, 2 * L, (0, y, z), "Hull alloy", rot=(0, 90, 0), segments=8)
    for x in (-L + 0.12, 0.0, L - 0.12):
        for s in (-1, 1):
            p.box((0.025, 0.025, 0.56), (x, 0, z / 2), "Hull alloy", rot=(s * 38, 0, 0))
        p.cyl(0.012, 2 * w, (x, 0, z * 0.5), "Hull alloy", rot=(90, 0, 0), segments=6)
        for y in (-w, w):
            p.cyl(0.02, 0.02, (x, y * 1.05, 0.01), "Rubber", segments=6)
    p.cloth([(-L + 0.02, -w, z + 0.01), (L - 0.02, -w, z + 0.01), (L - 0.02, w, z + 0.01), (-L + 0.02, w, z + 0.01)],
            "Canvas", sag=0.06, ripple=0.006, thickness=0.006, cell=0.06, droop=(0, 1, 0, 1))
    p.cyl(0.08, 0.5, (-L + 0.2, 0, z + 0.04), "Bleached", rot=(90, 0, 0), segments=12)
    mat = _blanket(r)
    p.cloth([(0.2, -w + 0.02, z + 0.06), (0.75, -w + 0.02, z + 0.06), (0.75, w - 0.02, z + 0.06), (0.2, w - 0.02, z + 0.06)],
            mat, sag=-0.03, ripple=0.012, thickness=0.05, cell=0.06, seed=5)
    p.collider((2 * L, 2 * w + 0.04, z + 0.03), (0, 0, (z + 0.03) / 2))
    return p


def locker(style: Style) -> Piece:
    """A steel locker: louvred door, a recessed handle and a padlock, the
    room's number on a plate, dented and repainted more than once."""
    p = Piece("locker", "prop", "0.5 × 0.5 × 1.9 m")
    r = rng(style, p.name)
    paint = r.choice(REPAINTS)
    p.span((-0.25, -0.24, 0.06), (0.25, 0.25, 1.9), paint)
    for x in (-0.23, 0.23):
        for y in (-0.22, 0.22):
            p.span((x - 0.02, y - 0.02, 0), (x + 0.02, y + 0.02, 0.07), "Hull dark")
    p.span((-0.23, -0.26, 0.1), (0.23, -0.24, 1.86), paint)
    for z0 in (0.2, 1.55):
        for k in range(6):
            p.box((0.28, 0.025, 0.012), (0, -0.262, z0 + k * 0.035), "Hull dark", rot=(35, 0, 0))
    p.span((0.12, -0.28, 0.85), (0.18, -0.26, 1.05), "Hull dark")
    p.box((0.02, 0.03, 0.1), (0.15, -0.29, 0.95), "Hull alloy")
    p.torus(0.025, 0.006, (0.15, -0.305, 0.86), "Steel", rot=(90, 0, 0), segments=12, sides=4)
    p.span((0.13, -0.315, 0.8), (0.17, -0.295, 0.84), "Brass")
    for z in (0.3, 1.0, 1.7):
        p.cyl(0.012, 0.06, (-0.235, -0.26, z), "Hull dark", segments=6)
    p.stencil("20", (0, -0.265, 1.35), 0.1, "Stencil white")
    p.collider((0.5, 0.5, 1.9), (0, 0, 0.95))
    return p


def folding_table(style: Style) -> Piece:
    """A folding table: an edged top on X-legs, cups and a deck of cards on
    it, and a clip-on lamp. 0.75 m high: vaultable."""
    p = Piece("folding_table", "prop", "1.2 × 0.7 × 0.75 m: vaultable")
    r = rng(style, p.name)
    z = 0.75
    p.span((-0.6, -0.35, z - 0.03), (0.6, 0.35, z), "Wood")
    for y in (-0.35, 0.35):
        p.span((-0.6, y - 0.008, z - 0.07), (0.6, y + 0.008, z - 0.01), "Hull alloy")
    for x in (-0.6, 0.6):
        p.span((x - 0.008, -0.35, z - 0.07), (x + 0.008, 0.35, z - 0.01), "Hull alloy")
    for x in (-0.45, 0.45):
        for s in (-1, 1):
            p.box((0.025, 0.025, 0.86), (x, 0, z / 2 - 0.02), "Hull alloy", rot=(s * 30, 0, 0))
        p.cyl(0.01, 0.6, (x, 0, z * 0.45), "Hull alloy", rot=(90, 0, 0), segments=6)
    for i, (x, y) in enumerate(((-0.3, -0.1), (-0.1, 0.12), (0.2, -0.05))):
        p.lathe([(0, 0), (0.035, 0), (0.04, 0.09), (0.035, 0.09), (0.03, 0.005), (0, 0.005)], (x, y, z), r.choice(["Hull alloy", "Repaint cream", "Crew orange"]), segments=12)
    with p.painted():
        for k in range(5):
            p.box((0.06, 0.09, 0.003), (0.3 + k * 0.02, 0.1 + k * 0.01, z + 0.005 + k * 0.0005), "Paper", rot=(0, 0, k * 9))
    # The lamp: a clamp, an arm and a shade.
    p.span((0.48, 0.25, z - 0.06), (0.56, 0.33, z + 0.03), "Hull dark")
    p.cyl(0.01, 0.3, (0.52, 0.29, z + 0.15), "Hull dark", segments=6)
    p.cyl(0.01, 0.2, (0.45, 0.29, z + 0.3), "Hull dark", rot=(0, 70, 0), segments=6)
    p.lathe([(0.0, 0.0), (0.03, 0.0), (0.07, -0.08), (0.0, -0.08)], (0.36, 0.29, z + 0.33), "Crew orange", segments=12)
    p.sphere(0.025, (0.36, 0.29, z + 0.26), "Sodium lamp", segments=8, rings=6)
    p.collider((1.2, 0.7, z), (0, 0, z / 2))
    return p


def stool(style: Style) -> Piece:
    """A steel stool, worn bare where people sit."""
    return Sourced("stool", "prop", "0.45 m across, 0.46 m high; no collider", asset="metal_stool_02", res="1k", height=0.46)


def curtain_partition(style: Style) -> Piece:
    """A curtain on a rail between two posts: cloth walls are what the
    Stacks' rooms are made of. Hangs in pleats from rings on the rail."""
    p = Piece("curtain_partition", "prop", "2 m curtain on a rail, 2.1 m high")
    r = rng(style, p.name)
    top = 2.05
    for x in (-H + 0.03, H - 0.03):
        p.cyl(0.02, top + 0.08, (x, 0, (top + 0.08) / 2), "Hull dark", segments=8)
        p.span((x - 0.1, -0.1, 0), (x + 0.1, 0.1, 0.02), "Hull dark")
    p.cyl(0.015, 2 * H - 0.06, (0, 0, top + 0.05), "Hull alloy", rot=(0, 90, 0), segments=8)
    main, stripe = r.sample(["Fabric red", "Fabric teal", "Fabric ochre", "Fabric indigo", "Canvas", "Fabric olive"], 2)
    n = 18
    for i in range(n):
        x0 = -H + 0.06 + i * (2 * H - 0.12) / n
        x1 = x0 + (2 * H - 0.12) / n
        y0, y1 = (0.05, -0.05) if i % 2 else (-0.05, 0.05)
        hem = 0.04 + 0.03 * r.random()
        mat = stripe if 7 <= i <= 9 else main
        p.cloth([(x0, y0, top), (x1, y1, top), (x1, y1, hem), (x0, y0, hem)], mat, sag=0.0, ripple=0.0,
                thickness=0.006, cell=0.2)
        p.torus(0.025, 0.005, (x0, 0, top + 0.05), "Brass", rot=(0, 90, 0), segments=10, sides=4)
    # Placed by the dozen through the Stacks: the game draws pleats of flat
    # board, the cloth baked onto them.
    with p.lowpoly():
        for i in range(n):
            x0 = -H + 0.06 + i * (2 * H - 0.12) / n
            x1 = x0 + (2 * H - 0.12) / n
            y0, y1 = (0.05, -0.05) if i % 2 else (-0.05, 0.05)
            p.cloth([(x0, y0, top), (x1, y1, top), (x1, y1, 0.04), (x0, y0, 0.04)], stripe if 7 <= i <= 9 else main,
                    sag=0.0, ripple=0.0, thickness=0.006, cell=3.0)
        p.cyl(0.015, 2 * H - 0.06, (0, 0, top + 0.05), "Hull alloy", rot=(0, 90, 0), segments=4)
        for x in (-H + 0.03, H - 0.03):
            p.span((x - 0.02, -0.02, 0), (x + 0.02, 0.02, top + 0.08), "Hull dark")
            p.span((x - 0.1, -0.1, 0), (x + 0.1, 0.1, 0.02), "Hull dark")
    p.collider((GRID, 0.15, 2.1), (0, 0, 1.05))
    return p


def cabin_door(style: Style) -> Piece:
    """A 2 m hull wall with a room's doorway, 1 × 2.1 m: a frame with a
    sliding door run open behind the wall, a room number over it, and a
    curtain hung in the gap for what privacy there is. Tiled through the
    Stacks, so the game draws a low stand-in with all this baked onto it."""
    p = Piece("cabin_door", "kit", "2 m wall, 1 × 2.1 m doorway, room number")
    r = rng(style, p.name)
    half, top = 0.5, 2.1
    _wall(p, style, (-half, half), top)
    # The door frame, proud of both faces, and the leaf's track above.
    for s in (-1, 1):
        p.span((-half - 0.08, s * 0.1 if s > 0 else -0.17, 0), (-half, 0.17 if s > 0 else -0.1, top + 0.08), "Crew grey")
        p.span((half, s * 0.1 if s > 0 else -0.17, 0), (half + 0.08, 0.17 if s > 0 else -0.1, top + 0.08), "Crew grey")
        p.span((-half - 0.08, s * 0.1 if s > 0 else -0.17, top), (half + 0.08, 0.17 if s > 0 else -0.1, top + 0.08), "Crew grey")
    p.span((-half - 0.1, -0.21, top + 0.09), (H - 0.14, -0.17, top + 0.13), "Hull dark")
    _bolts(p, [(x, -0.21, top + 0.11) for x in (-0.5, 0.0, 0.5)], r=0.008, h=0.008)
    # The room's number on a plate over the door.
    p.span((-0.3, -0.16, top + 0.25), (0.3, -0.13, top + 0.5), "Hull dark")
    # The curtain in the doorway, on a wire.
    p.cable((-half, -0.03, top - 0.05), (half, -0.03, top - 0.05), 0.004, "Steel", steps=2, segments=5)
    curtain = r.choice(["Fabric red", "Fabric teal", "Canvas", "Fabric indigo"])
    for i in range(6):
        x0 = -half + 0.02 + i * (2 * half - 0.04) / 6
        x1 = x0 + (2 * half - 0.04) / 6
        y0, y1 = (-0.06, 0.0) if i % 2 else (0.0, -0.06)
        p.cloth([(x0, y0, top - 0.06), (x1, y1, top - 0.06), (x1, y1, 0.08), (x0, y0, 0.08)], curtain, sag=0, ripple=0,
                thickness=0.006, cell=0.3)
    p.stencil("221", (0, -0.166, top + 0.375), 0.16, "Stencil white")
    with p.lowpoly():
        p.span((-H, -0.15, 0), (-half - 0.08, 0.15, DECK), "Hull alloy")
        p.span((half + 0.08, -0.15, 0), (H, 0.15, DECK), "Hull alloy")
        p.span((-half - 0.08, -0.15, top + 0.08), (half + 0.08, 0.15, DECK), "Hull alloy")
        for x0, x1 in ((-half - 0.08, -half), (half, half + 0.08)):
            p.span((x0, -0.17, 0), (x1, 0.17, top + 0.08), "Crew grey")
        p.span((-half - 0.08, -0.17, top), (half + 0.08, 0.17, top + 0.08), "Crew grey")
        p.span((-half, -0.06, 0.08), (half, 0.0, top - 0.06), curtain)
    p.collider((H - half + SEAM / 2, 0.3, DECK), (-(H + half + SEAM / 2) / 2, 0, DECK / 2))
    p.collider((H - half + SEAM / 2, 0.3, DECK), ((H + half + SEAM / 2) / 2, 0, DECK / 2))
    p.collider((2 * half, 0.3, DECK - top), (0, 0, (DECK + top) / 2))
    return p


def laundry_line(style: Style) -> Piece:
    """A line strung between two posts, hung with whatever's been washed:
    shirts, a towel, a blanket, pegged on and dripping."""
    p = Piece("laundry_line", "prop", "3 m line, 1.9 m posts; no collider")
    r = rng(style, p.name)
    top = 1.88
    for x in (-1.6, 1.6):
        p.cyl(0.025, top + 0.05, (x, 0, (top + 0.05) / 2), "Hull dark", segments=8)
        p.span((x - 0.14, -0.14, 0), (x + 0.14, 0.14, 0.03), "Hull dark")
    p.cable((-1.6, 0, top), (1.6, 0, top), 0.005, "Bleached", sag=0.12, steps=4, segments=5)
    x = -1.35
    while x < 1.2:
        wdt = r.uniform(0.3, 0.55)
        drop = r.uniform(0.45, 0.85)
        t0, t1 = (x + 1.6) / 3.2, (x + wdt + 1.6) / 3.2
        z0 = top - 0.12 * 4 * t0 * (1 - t0) - 0.01
        z1 = top - 0.12 * 4 * t1 * (1 - t1) - 0.01
        mat = r.choice(["Fabric red", "Fabric teal", "Bleached", "Fabric ochre", "Canvas", "Fabric indigo"])
        p.cloth([(x, -0.01, z0), (x + wdt, 0.01, z1), (x + wdt, 0.01, z1 - drop), (x, -0.01, z0 - drop)], mat,
                sag=0.0, ripple=0.0, thickness=0.006, cell=0.3)
        for xx, zz in ((x + 0.03, z0), (x + wdt - 0.03, z1)):
            p.box((0.012, 0.02, 0.06), (xx, 0, zz), "Wood")
        x += wdt + r.uniform(0.08, 0.2)
    return p


def cooker(style: Style) -> Piece:
    """A two-ring gas cooker on a welded stand, its bottle beside it on a
    hose, a kettle on one ring and a pot on the other: a Stacks kitchen."""
    p = Piece("cooker", "prop", "0.6 × 0.45 × 0.95 m with the kettle and the bottle", budget=35000)
    z = 0.68
    for x in (-0.28, 0.28):
        for y in (-0.2, 0.2):
            p.span((x - 0.015, y - 0.015, 0), (x + 0.015, y + 0.015, z), "Hull dark")
    for zz in (0.2, z - 0.02):
        p.span((-0.3, -0.22, zz - 0.02), (0.3, 0.22, zz), "Hull dark")
    p.span((-0.28, -0.2, 0.2), (0.28, 0.2, 0.21), "Grating")
    p.span((-0.27, -0.19, z), (0.27, 0.19, z + 0.08), "Repaint cream")
    p.span((-0.26, -0.2, z + 0.01), (0.26, -0.19, z + 0.07), "Hull alloy")
    for x in (-0.13, 0.13):
        p.torus(0.07, 0.008, (x, 0, z + 0.085), "Hull dark", segments=16, sides=4)
        p.cyl(0.03, 0.01, (x, 0, z + 0.085), "Hull dark", segments=10)
        for k in range(4):
            p.box((0.15, 0.01, 0.01), (x, 0, z + 0.095), "Hull dark", rot=(0, 0, k * 45))
        p.cyl(0.015, 0.02, (x, -0.21, z + 0.04), "Hull dark", rot=(90, 0, 0), segments=8)
    p.source("vintage_electric_kettle", at=(-0.13, 0.0, z + 0.1), height=0.25, rot=(0, 0, 30))
    p.lathe([(0, 0), (0.09, 0), (0.1, 0.12), (0.105, 0.125), (0.095, 0.125), (0.09, 0.01), (0, 0.01)], (0.13, 0, z + 0.1), "Medical white", segments=16)
    p.cyl(0.098, 0.02, (0.13, 0, z + 0.235), "Medical white", segments=16)
    p.sphere(0.02, (0.13, 0, z + 0.255), "Hull dark", segments=6, rings=4)
    for sx in (-1, 1):
        p.box((0.05, 0.015, 0.015), (0.13 + sx * 0.12, 0, z + 0.2), "Hull dark")
    p.source("small_lpg_tank", at=(0.47, 0.05, 0), height=0.6, recolour="Crew orange")
    p.cable((0.42, 0.05, 0.6), (0.27, 0.1, z + 0.03), 0.01, "Rubber", sag=0.15, steps=4, segments=5)
    p.collider((0.6, 0.45, 0.68), (0, 0, 0.34))
    return p


# Warehouses ----------------------------------------------------------------

def _pallet(p: Piece, at, w=1.2, d=1.0) -> None:
    """A wooden pallet: top boards with gaps, nine blocks, bottom boards."""
    x0, y0, z0 = at
    for i in range(7):
        x = x0 - w / 2 + 0.05 + i * (w - 0.1) / 6
        p.span((x - 0.05, y0 - d / 2, z0 + 0.12), (x + 0.05, y0 + d / 2, z0 + 0.142), "Wood")
    for y in (-d / 2 + 0.05, 0, d / 2 - 0.05):
        p.span((x0 - w / 2, y0 + y - 0.05, z0 + 0.1), (x0 + w / 2, y0 + y + 0.05, z0 + 0.12), "Wood dark")
        for x in (-w / 2 + 0.05, 0, w / 2 - 0.05):
            p.span((x0 + x - 0.05, y0 + y - 0.05, z0 + 0.022), (x0 + x + 0.05, y0 + y + 0.05, z0 + 0.1), "Wood")
    for x in (-w / 2 + 0.05, 0, w / 2 - 0.05):
        p.span((x0 + x - 0.05, y0 - d / 2, z0), (x0 + x + 0.05, y0 + d / 2, z0 + 0.022), "Wood dark")


def storage_rack(style: Style) -> Piece:
    """A bay of pallet racking: punched uprights, orange beams and wire
    decks, stocked with crates, boxes and a gas bottle on pallets, the bay's
    number on a beam."""
    p = Piece("storage_rack", "prop", "2 × 1.1 × 3 m racking, stocked", budget=STALL)
    w, d, top = 1.0, 0.55, 3.0
    for x in (-w + 0.04, w - 0.04):
        for y in (-d + 0.04, d - 0.04):
            p.span((x - 0.04, y - 0.04, 0), (x + 0.04, y + 0.04, top), "Container blue")
            p.span((x - 0.07, y - 0.07, 0), (x + 0.07, y + 0.07, 0.01), "Hull dark")
        for k in range(6):
            z = 0.25 + k * 0.5
            p.box((0.03, 2 * d - 0.1, 0.03), (x, 0, z), "Container blue", rot=(30 if k % 2 else -30, 0, 0))
        with p.painted():
            for k in range(40):
                p.span((x - 0.015, -d - 0.008, 0.1 + k * 0.07), (x + 0.015, -d - 0.005, 0.13 + k * 0.07), "Hull dark")
    levels = (0.0, 1.0, 2.0)
    r = rng(style, p.name)
    for i, z in enumerate(levels):
        if z:
            for y in (-d + 0.04, d - 0.04):
                p.span((-w, y - 0.05, z - 0.12), (w, y + 0.05, z), "Crew orange")
            p.span((-w + 0.08, -d + 0.08, z - 0.01), (w - 0.08, d - 0.08, z), "Grating")
        _pallet(p, (0, 0, z + (0.0 if z == 0 else 0.0)), w=1.2, d=1.0)
        base = z + 0.142
        if i == 0:
            p.source("plastic_container", at=(0, 0, base), length=0.9)
            p.source("plastic_crate_02", at=(0.2, -0.05, base + 0.43), height=0.25)
        elif i == 1:
            p.source("cardboard_box_01", at=(-0.3, 0.0, base), height=0.34, rot=(0, 0, 90))
            # Boxes of the Crew's own, taped and stencilled.
            for k, (x, y, hh) in enumerate(((0.3, -0.2, 0.4), (0.3, 0.25, 0.3), (-0.3, 0.1, 0.3))):
                zb = base + (0.34 if k == 2 else 0.0)
                p.span((x - 0.22, y - 0.2, zb), (x + 0.22, y + 0.2, zb + hh), "Repaint cream" if k != 1 else "Crew orange")
                with p.painted():
                    p.span((x - 0.05, y - 0.206, zb), (x + 0.05, y - 0.204, zb + hh), "Paper aged")
        else:
            # Sacks of grain, slumped.
            for k, (x, y) in enumerate(((-0.3, -0.15), (0.25, -0.15), (-0.05, 0.25))):
                p.sphere(0.25, (x, y, base + 0.15), "Canvas", scale=(1.0, 0.75, 0.6), rot=(0, 0, 20 * k), segments=10, rings=8)
    with p.painted():
        p.stencil("3-26", (0, -d - 0.016, 1.94), 0.07, "Stencil white")
        p.stencil("3-27", (0, -d - 0.016, 0.94), 0.07, "Stencil white")
    p.collider((2 * w, 2 * d, top), (0, 0, top / 2))
    return p


def pallet(style: Style) -> Piece:
    """A wooden pallet on its own: the Pads' common currency of loading."""
    p = Piece("pallet", "prop", "1.2 × 1.0 × 0.144 m; walked over")
    _pallet(p, (0, 0, 0))
    with p.painted():
        p.stencil("17", (0, -0.506, 0.06), 0.04, "Stencil white")
    return p


def roller_door(style: Style) -> Piece:
    """A warehouse's 2 m wall piece with a roll-up shutter, 1.7 × 2.8 m,
    shut and padlocked: the drum's housing above, guide rails, ribbed slats
    and a striped bottom bar. Tiled along warehouse fronts, so the game draws
    a low stand-in with all this baked onto it."""
    p = Piece("roller_door", "kit", "2 × 3.6 m wall piece, 1.7 × 2.8 m shutter, closed")
    half, top = 0.85, 2.8
    _wall(p, style, (-half, half), top + 0.4)
    # The shutter: slats with a rib each, across the opening.
    p.span((-half, -0.02, 0), (half, 0.02, top + 0.4), "Hull alloy")
    for k in range(28):
        z = 0.06 + k * 0.1
        p.span((-half, -0.035, z), (half, -0.02, z + 0.07), "Container blue")
        p.span((-half, -0.042, z + 0.03), (half, -0.035, z + 0.04), "Container blue")
    # Guide rails, the drum's housing, the bottom bar.
    for x0, x1 in ((-half - 0.08, -half + 0.02), (half - 0.02, half + 0.08)):
        p.span((x0, -0.106, 0.005), (x1, -0.025, top + 0.4), "Hull dark")
    p.span((-half - 0.12, -0.26, top + 0.405), (half + 0.12, -0.05, top + 0.75), "Crew grey")
    p.cyl(0.1, 0.12, (half + 0.06, -0.18, top + 0.57), "Hull dark", rot=(0, 90, 0), segments=12)
    _bolts(p, [(x, -0.26, top + 0.57) for x in (-0.8, -0.4, 0, 0.4, 0.8)])
    p.span((-half, -0.07, 0.0), (half, -0.03, 0.08), "Hull dark")
    p.box((0.18, 0.04, 0.03), (0, -0.09, 0.06), "Hull alloy")
    p.torus(0.03, 0.007, (0.3, -0.08, 0.04), "Steel", rot=(90, 0, 0), segments=12, sides=4)
    p.span((0.28, -0.1, -0.0), (0.32, -0.08, 0.02), "Brass")
    _hazard(p, -half, half, 0.0, 0.08, -0.07, width=0.08, base="Hazard yellow")
    p.stencil("28", (0, -0.266, top + 0.57), 0.2, "Stencil white")
    p.stencil("8", (0, -0.05, 1.45), 0.7, "Stencil white")
    with p.lowpoly():
        p.span((-H, -0.15, 0), (-half - 0.08, 0.15, DECK), "Hull alloy")
        p.span((half + 0.08, -0.15, 0), (H, 0.15, DECK), "Hull alloy")
        p.span((-half - 0.08, -0.15, top + 0.4), (half + 0.08, 0.15, DECK), "Hull alloy")
        p.span((-half - 0.08, -0.1, 0), (half + 0.08, 0.04, top + 0.4), "Container blue")
        p.span((-half - 0.12, -0.26, top + 0.4), (half + 0.12, -0.15, top + 0.75), "Crew grey")
    p.collider((GRID + SEAM, 0.3, DECK), (0, 0, DECK / 2))
    return p


# The market's other stalls ---------------------------------------------------

def _stall_frame(p: Piece, w: float, front_z: float, back_z: float) -> None:
    """Four poles, braced, to hang a roof or awning from."""
    for x in (-w, w):
        for y, h in ((-0.98, front_z), (0.88, back_z)):
            p.cyl(0.028, h, (x, y, h / 2), "Hull dark", segments=8)
            p.span((x - 0.07, y - 0.07, 0), (x + 0.07, y + 0.07, 0.015), "Hull dark")
        p.cyl(0.015, 1.9, (x, -0.05, (front_z + back_z) / 2 - 0.25), "Hull dark", rot=(90 - math.degrees(math.atan2(back_z - front_z, 1.86)), 0, 0), segments=6)


def food_stall(style: Style) -> Piece:
    """A food stall for a 2 m bay: a panelled counter of produce in crates
    and bowls, a cooking table behind with a steamer and pots on the boil,
    string lights, and a striped awning sagging between its poles."""
    p = Piece("food_stall", "prop", "2 × 2 m stall, counter 0.9 m, awning to 2.5 m", budget=STALL)
    r = rng(style, p.name)
    main, stripe = FABRICS[style.fabric]
    w = 0.95
    # The counter: a frame of panels, its top edged.
    p.span((-w, -0.95, 0.04), (w, -0.45, 0.86), r.choice(REPAINTS))
    for x in (-w, -0.32, 0.32, w):
        p.span((x - 0.03, -0.97, 0), (x + 0.03, -0.43, 0.86), "Wood dark")
    for x0, x1 in ((-w + 0.05, -0.37), (-0.27, 0.27), (0.37, w - 0.05)):
        p.span((x0, -0.97, 0.12), (x1, -0.955, 0.78), r.choice(REPAINTS + ["Wood"]))
    p.span((-w - 0.03, -1.0, 0.86), (w + 0.03, -0.42, 0.9), "Wood")
    p.collider((2 * w + 0.06, 0.58, 0.9), (0, -0.71, 0.45))
    # Produce in crates and bowls along the counter.
    # Produce: a crate heaped with tomatoes, a bowl of greens, roots in a basket.
    p.source("plastic_crate_02", at=(-0.6, -0.72, 0.9), length=0.42)
    for k in range(14):
        p.sphere(0.035 + 0.008 * r.random(), (-0.76 + (k % 5) * 0.075, -0.8 + (k // 5) * 0.075, 1.08 + 0.02 * r.random()),
                 r.choice(["Tomato", "Veg red"]), segments=5, rings=3)
    p.lathe([(0, 0), (0.1, 0.0), (0.15, 0.07), (0.14, 0.075), (0.09, 0.012), (0, 0.012)], (0.05, -0.72, 0.9), "Wood", segments=16)
    for k in range(9):
        p.sphere(0.045, (-0.03 + (k % 3) * 0.06, -0.78 + (k // 3) * 0.06, 0.99), r.choice(["Veg green", "Foliage light", "Crop green"]),
                 scale=(1, 1, 0.7), segments=5, rings=3)
    p.span((0.38, -0.86, 0.9), (0.78, -0.58, 0.92), "Wood dark")
    for k in range(10):
        p.sphere(0.04, (0.43 + (k % 5) * 0.08, -0.82 + (k // 5) * 0.1, 0.95), r.choice(["Veg orange", "Wheat", "Fabric ochre"]),
                 scale=(1.6, 0.8, 0.8), rot=(0, 0, 20 * k), segments=5, rings=3)
    # The cooking table behind: a hot plate, a steamer, two pots.
    p.span((-w + 0.05, 0.3, 0.76), (w - 0.05, 0.85, 0.8), "Hull alloy")
    for x in (-w + 0.1, w - 0.1):
        for y in (0.35, 0.8):
            p.span((x - 0.02, y - 0.02, 0), (x + 0.02, y + 0.02, 0.76), "Hull dark")
    p.collider((2 * w, 0.65, 0.8), (0, 0.525, 0.4))
    p.span((-0.7, 0.4, 0.8), (-0.1, 0.75, 0.86), "Hull dark")
    for i in range(3):
        p.lathe([(0, 0), (0.16, 0), (0.17, 0.1), (0.15, 0.1), (0.15, 0.01), (0, 0.01)], (-0.4, 0.57, 0.86 + i * 0.1), "Wood", segments=16)
    p.lathe([(0, 0), (0.17, 0.0), (0.1, 0.08), (0, 0.09)], (-0.4, 0.57, 1.16), "Wood", segments=16)
    p.sphere(0.1, (-0.4, 0.57, 1.32), "Steam", scale=(1.2, 1.2, 0.8), segments=10, rings=6)
    p.source("brass_pot_01", at=(0.15, 0.55, 0.8), height=0.25)
    p.source("pot_enamel_01", at=(0.6, 0.55, 0.8), height=0.17, rot=(0, 0, 40))
    # Poles, the awning in stripes, string lights along the front.
    _stall_frame(p, w, 2.2, 2.5)
    n = 8
    for i in range(n):
        x0 = -w - 0.1 + (2 * w + 0.2) * i / n
        x1 = x0 + (2 * w + 0.2) / n
        p.cloth([(x0, -1.08, 2.22), (x1, -1.08, 2.22), (x1, 0.95, 2.52), (x0, 0.95, 2.52)],
                main if i % 2 == 0 else stripe, sag=0.07, ripple=0.01, thickness=0.006, cell=0.1, seed=0,
                droop=(0.4, 0, 0.2, 0))
        p.cloth([(x0, -1.08, 2.22), (x1, -1.08, 2.22), (x1, -1.1, 1.98), (x0, -1.1, 1.98)],
                stripe if i % 2 == 0 else main, sag=0.0, ripple=0.0, thickness=0.006, cell=0.3)
    p.cable((-w, -1.0, 2.1), (w, -1.0, 2.1), 0.004, "Rubber", sag=0.15, steps=4, segments=5)
    for k in range(7):
        t = (k + 0.5) / 7
        p.sphere(0.022, (-w + 2 * w * t, -1.0, 2.1 - 0.15 * 4 * t * (1 - t) - 0.03), "Sodium lamp", segments=5, rings=3)
    with p.painted():
        p.span((-0.25, -0.976, 0.3), (0.25, -0.974, 0.6), "Paper")
    p.stencil("10", (0, -0.981, 0.45), 0.12, "Fabric red")
    return p


def parts_stall(style: Style) -> Piece:
    """A parts stall: bins of salvage on the counter, a pegboard of tools at
    the back, a wheel rim and a tyre propped against the front, under a roof
    of corrugated sheet. Everything in the Hull is somebody's spare."""
    p = Piece("parts_stall", "prop", "2 × 2 m stall, counter 0.9 m, roof to 2.5 m", budget=STALL)
    r = rng(style, p.name)
    w = 0.95
    p.span((-w, -0.95, 0.04), (w, -0.45, 0.86), "Crew grey")
    for x in (-w, 0.0, w):
        p.span((x - 0.03, -0.97, 0), (x + 0.03, -0.43, 0.86), "Hull dark")
    p.span((-w - 0.03, -1.0, 0.86), (w + 0.03, -0.42, 0.9), "Hull alloy")
    p.collider((2 * w + 0.06, 0.58, 0.9), (0, -0.71, 0.45))
    # Bins of parts.
    for i, x in enumerate((-0.65, 0.35)):
        p.source("plastic_crate_02", at=(x, -0.72, 0.9), length=0.45, recolour=r.choice(["Container blue", "Crew orange", "Container green"]))
    # A tray of loose bolts and nuts between them.
    p.span((-0.32, -0.88, 0.9), (0.12, -0.56, 0.94), "Hull dark")
    for k in range(14):
        p.cyl(0.012, 0.012, (-0.28 + (k % 7) * 0.06, -0.8 + (k // 7) * 0.12, 0.946), r.choice(["Steel", "Brass", "Rust"]), segments=6)
    p.source("can_rusted", at=(-0.7, -0.7, 0.95), height=0.12)
    p.source("screwdrivers_02", at=(0.75, -0.72, 0.9), width=0.27)
    # The pegboard and its tools.
    p.span((-w + 0.05, 0.8, 0.9), (w - 0.05, 0.86, 2.1), "Wood")
    for x in (-w + 0.05, w - 0.05):
        p.span((x - 0.03, 0.82, 0), (x + 0.03, 0.88, 2.15), "Hull dark")
    with p.painted():
        for i in range(17):
            for k in range(11):
                p.cyl(0.006, 0.002, (-0.85 + i * 0.106, 0.795, 0.95 + k * 0.108), "Wood dark", rot=(90, 0, 0), segments=4)
    p.collider((2 * w, 0.1, 2.1), (0, 0.85, 1.05))
    for x, asset, rot, kw in ((-0.25, "adjustable_wrench", (0, 0, 0), dict(height=0.25)),
                              (0.1, "combination_wrench", (90, 0, 0), dict(height=0.3)),
                              (0.45, "pliers", (0, 0, 0), dict(height=0.18))):
        p.source(asset, at=(x, 0.75, 1.45), rot=rot, **kw)
    # A wheel rim propped against the front, made here (the sourced one's too heavy to share a stall).
    p.torus(0.19, 0.03, (-0.55, -1.03, 0.2), "Rust", rot=(78, 0, 0), segments=20, sides=6)
    p.cyl(0.13, 0.08, (-0.55, -1.03, 0.2), "Rust", rot=(78, 0, 0), segments=16)
    for k in range(5):
        a = math.tau * k / 5
        p.cyl(0.015, 0.02, (-0.55 + 0.08 * math.cos(a), -1.08, 0.2 + 0.08 * math.sin(a)), "Hull dark", rot=(78, 0, 0), segments=4)
    p.source("old_tyre", at=(0.6, -1.06, 0), height=0.55, rot=(0, 0, 10))
    # Poles and a corrugated roof.
    _stall_frame(p, w, 2.2, 2.5)
    a = math.degrees(math.atan2(0.3, 1.86 + 0.4))
    n = 14
    for i in range(n):
        x = -w - 0.1 + (2 * w + 0.2) * (i + 0.5) / n
        p.box(((2 * w + 0.2) / n, 2.3, 0.012), (x, -0.06, 2.36 + (0.02 if i % 2 else 0)), r.choice(["Hull alloy", "Rust", "Crew grey"]) if i % 5 == 0 else "Hull alloy", rot=(a, 0, 0))
    p.stencil("30", (-0.45, -0.956, 0.5), 0.15, "Stencil white")
    return p


PIECES = [air_scrubber, air_fan, pump_unit, valve_station, junction_box, power_conduit, generator, water_tank,
          control_console, hazard_barrier, bunk_bed, cot, locker, folding_table, stool, curtain_partition,
          cabin_door, laundry_line, cooker, storage_rack, pallet, roller_door, food_stall, parts_stall]
