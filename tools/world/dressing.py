"""Signs and dressing: what tells a player where they are and whose ground
it is, and the cloth that's everywhere because cloth is cheap and metal
isn't (docs/look-and-feel.md). District signs with real lettering, Crew
direction plates stencilled on the decks, hand-painted shop signs hanging
off wall brackets, marking panels, and rugs, banners, awnings and bunting.

Placement:
- Wall-mounted pieces stand with their origin on the floor at the foot of
  the wall and their back at y = 0, on the wall's face, at the height they
  belong (shop signs at about 2.9 m, direction plates at about 2.2 m).
- Free-standing signs stand on their origin like any prop.
- Rugs lie on the floor with their tops 1.4 cm up.
- Things strung across a street (street_awning, bunting) have their origin
  on the street's middle, and their ends at the walls (x = ±3 m and ±4 m).
- doorway_curtain hangs in a 1 m doorway centred on its origin.

Lettering is Blender's font (kit.Piece.lettering): raised brass and enamel
as geometry, left plain so the finish doesn't bevel every letter, and
everything flat as paint, baked in."""
import math

from mathutils import Vector

from kit import FABRICS, PALETTE, REPAINTS, Piece, Style, rng

CATEGORY = "Signs and dressing"

# Cloth colours, named as fabrics so the finish dirties them as cloth (not
# metal, whose edges wear through to steel).
PALETTE.setdefault("Fabric white", (0.74, 0.73, 0.68))
PALETTE.setdefault("Fabric navy", (0.03, 0.05, 0.15))
PALETTE.setdefault("Fabric orange", (0.72, 0.24, 0.04))
PALETTE.setdefault("Fabric grey", (0.22, 0.23, 0.24))
PALETTE.setdefault("Fabric charcoal", (0.07, 0.07, 0.08))
PALETTE.setdefault("Fabric brass", (0.58, 0.40, 0.13))
PALETTE.setdefault("Rope", (0.48, 0.38, 0.22))
PALETTE.setdefault("Enamel white", (0.84, 0.84, 0.82))
PALETTE.setdefault("Ink", (0.01, 0.01, 0.03))

SIGN, FABRIC = 8000, 6000

# Paint goes on in layers, each a millimetre thick and 6 mm out from the
# last: far enough apart that no two flicker, near enough (18 mm at most)
# for the finish's bake to reach. L1 is paint on the surface, L2 paint on
# paint, L3 paint on that.
L1, L2, L3, COAT = 0.006, 0.012, 0.018, 0.001
RUGS = ["Fabric red", "Fabric ochre", "Fabric indigo", "Fabric teal", "Fabric sand", "Fabric olive"]


# Shared parts ---------------------------------------------------------------

def _clip(poly, x0, z0, x1, z1):
    """A convex outline clipped to a rectangle (Sutherland-Hodgman)."""
    for axis, edge, keep_above in ((0, x0, True), (0, x1, False), (1, z0, True), (1, z1, False)):
        out = []
        for i, p in enumerate(poly):
            q = poly[(i + 1) % len(poly)]
            pin = p[axis] >= edge if keep_above else p[axis] <= edge
            qin = q[axis] >= edge if keep_above else q[axis] <= edge
            if pin:
                out.append(p)
            if pin != qin:
                t = (edge - p[axis]) / (q[axis] - p[axis])
                out.append((p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])))
        poly = out
        if len(poly) < 3:
            return []
    return poly


def _stripes(p: Piece, x0, z0, x1, z1, y, mat="Hull dark", width=0.08):
    """Hazard stripes painted on a -Y face at y: 45° bands, clipped to the
    rectangle (the rectangle itself is the plate's own colour)."""
    h = z1 - z0
    with p.painted():
        a = x0 - h
        while a < x1:
            band = _clip([(a, z0), (a + width, z0), (a + width + h, z1), (a + h, z1)], x0, z0, x1, z1)
            if band:
                p.prism(band, COAT, (0, y - L1, 0), mat)
            a += 2 * width


def _bolts(p: Piece, points, y, mat="Hull dark", r=0.011):
    """Bolt heads on a -Y face at y, at (x, z) points: plain, for there are
    many of them."""
    with p.plain():
        for x, z in points:
            p.cyl(r, 0.012, (x, y - 0.006, z), mat, rot=(90, 0, 0), segments=4)


def _rust(p: Piece, points, y, rand, length=0.12):
    """Rust run down from under bolts, painted on a -Y face at y."""
    with p.painted():
        for x, z in points:
            if rand.random() < 0.6:
                n = rand.uniform(0.5, 1.0) * length
                p.prism([(-0.006, 0), (0.006, 0), (0.002, -n), (-0.002, -n)], COAT, (x, y - L2, z - 0.012), "Rust")


def _text(p: Piece, text, x, z, h, y, mat, facing="-Y", align="CENTER", bold=0.0, spacing=1.0, layer=L1):
    """Painted lettering on a face at y (facing ±Y), layer out from it."""
    off = -layer if facing == "-Y" else layer
    with p.painted():
        p.lettering(text, (x, y + off, z), h, mat, facing=facing, depth=COAT, align=align, bold=bold, spacing=spacing)


def _raised(p: Piece, text, x, z, h, y, mat, depth=0.018, bold=0.0, spacing=1.0, align="CENTER"):
    """Letters standing out of a -Y face at y, depth deep (8 mm of it set
    into the face, so their backs are well inside it), left plain."""
    with p.plain():
        p.lettering(text, (x, y - depth / 2 + 0.008, z), h, mat, facing="-Y", depth=depth, bold=bold,
                    spacing=spacing, align=align)


ARROW = [(-0.3, 0), (0.3, 0), (0.3, 0.55), (0.75, 0.55), (0, 1), (-0.75, 0.55), (-0.3, 0.55)]


def _arrow(p: Piece, x, z, size, y, mat, way="up"):
    """A painted arrow on a -Y face at y, size high, pointing way."""
    turn = {"up": 0, "right": 90, "down": 180, "left": -90}[way]
    outline = [(a * size, (b - 0.5) * size) for a, b in ARROW]
    with p.painted():
        p.prism(outline, COAT, (x, y - L1, z), mat, rot=(0, turn, 0))


def _plate(p: Piece, x0, z0, x1, z1, depth, mat, rand, bolts=True):
    """A wall plate from x0 to x1 and z0 to z1, its back on the wall (y = 0)
    and its face depth out, bolted at the corners, rust run from the bolts."""
    p.span((x0, -depth, z0), (x1, 0, z1), mat)
    if bolts:
        pts = [(x0 + 0.035, z0 + 0.035), (x1 - 0.035, z0 + 0.035), (x0 + 0.035, z1 - 0.035), (x1 - 0.035, z1 - 0.035)]
        _bolts(p, pts, -depth)
        _rust(p, [(x, z) for x, z in pts if z > (z0 + z1) / 2], -depth, rand)


def _sheet(p: Piece, f, nu, nv, thickness, mat):
    """A sheet of cloth through f(u, v) (u, v in 0..1), thickness thick
    along its own normal, rimmed: for cloth kit's cloth() can't shape
    (hanging, bowed, cut)."""
    pts = [f(i / nu, j / nv) for j in range(nv + 1) for i in range(nu + 1)]
    w = nu + 1
    front, back = [], []
    for j in range(nv + 1):
        for i in range(nu + 1):
            du = pts[j * w + min(i + 1, nu)] - pts[j * w + max(i - 1, 0)]
            dv = pts[min(j + 1, nv) * w + i] - pts[max(j - 1, 0) * w + i]
            n = du.cross(dv)
            n = n.normalized() if n.length > 1e-9 else Vector((0, 0, 1))
            q = pts[j * w + i]
            front.append(p.bm.verts.new(q + n * thickness / 2))
            back.append(p.bm.verts.new(q - n * thickness / 2))
    faces = []
    for j in range(nv):
        for i in range(nu):
            k = (j * w + i, j * w + i + 1, (j + 1) * w + i + 1, (j + 1) * w + i)
            faces.append(p.bm.faces.new([front[a] for a in k]))
            faces.append(p.bm.faces.new([back[a] for a in reversed(k)]))
    ring = [i for i in range(nu)] + [nu + j * w for j in range(nv)] + \
           [nv * w + nu - i for i in range(nu)] + [(nv - j) * w for j in range(nv)]
    for a, b in zip(ring, ring[1:] + ring[:1]):
        faces.append(p.bm.faces.new((front[b], front[a], back[a], back[b])))
    p._faces(faces, mat, smooth=True)


def _seal(p: Piece, x, z, y, r):
    """The Registrars' seal on a -Y face at y: a brass disc, a notched rim, a
    navy field and the open book of the record in brass."""
    p.cyl(r, 0.03, (x, y - 0.015, z), "Brass", rot=(90, 0, 0), segments=16)
    p.torus(r - 0.01, 0.012, (x, y - 0.03, z), "Brass", rot=(90, 0, 0), segments=16, sides=4)
    p.cyl(r * 0.72, 0.012, (x, y - 0.036, z), "Repaint navy", rot=(90, 0, 0), segments=14)
    with p.plain():
        for i in range(16):
            a = math.tau * i / 16
            p.box((0.02, 0.012, 0.03), (x + math.cos(a) * r * 0.86, y - 0.036, z + math.sin(a) * r * 0.86),
                  "Brass", rot=(0, -math.degrees(a) + 90, 0))
        for s in (-1, 1):
            page = [(0, -0.4), (s * 0.45, -0.34), (s * 0.5, 0.42), (0, 0.36)]
            page = [(a * r * 0.55, b * r * 0.55) for a, b in page]
            p.prism(page if s > 0 else list(reversed(page)), 0.014, (x, y - 0.046, z), "Brass")


# District signs ---------------------------------------------------------------

def sign_exchange(style: Style) -> Piece:
    """The Exchange's sign over the floor's entrance in the old cargo bay:
    a navy board in a brass frame, EXCHANGE in raised brass, the Registrars'
    seal at each end and two gooseneck lamps over it. Wall-mounted: back at
    y = 0, origin on the floor below; the board runs 2.55–3.45 m up, clear
    of a 2.2 m doorway."""
    p = Piece("sign_exchange", "prop", "3.5 m sign, 2.55–3.8 m up; wall-mounted, no collider", budget=SIGN)
    r = rng(style, p.name)
    x, z0, z1, d = 1.7, 2.55, 3.45, 0.08
    p.span((-x, -d, z0), (x, 0, z1), "Repaint navy")
    # The frame, proud of the board and clear of the wall.
    for a, b in (((-x - 0.06, -d - 0.03, z1 - 0.01), (x + 0.06, -0.01, z1 + 0.06)),
                 ((-x - 0.06, -d - 0.03, z0 - 0.06), (x + 0.06, -0.01, z0 + 0.01)),
                 ((-x - 0.06, -d - 0.03, z0 + 0.012), (-x + 0.01, -0.01, z1 - 0.012)),
                 ((x - 0.01, -d - 0.03, z0 + 0.012), (x + 0.06, -0.01, z1 - 0.012))):
        p.span(a, b, "Brass")
    _bolts(p, [(sx * (x + 0.025), zz) for sx in (-1, 1) for zz in (z0 + 0.1, (z0 + z1) / 2, z1 - 0.1)], -d - 0.03, "Hull dark")
    _raised(p, "EXCHANGE", 0, 3.06, 0.25, -d, "Brass", depth=0.024, bold=0.02, spacing=1.1)
    _text(p, "FILED IS LEGAL", 0, 2.72, 0.07, -d, "Paper", spacing=1.3)
    for s in (-1, 1):
        _seal(p, s * 1.42, 3.0, -d, 0.22)
    # Hangers to the wall, and two gooseneck lamps over the board.
    for s in (-1, 1):
        p.span((s * 1.2 - 0.03, -0.14, z1 + 0.06), (s * 1.2 + 0.03, 0, z1 + 0.1), "Hull dark")
        p.tube([(s * 1.0, 0, 3.72), (s * 1.0, -0.2, 3.76), (s * 1.0, -0.42, 3.66)], 0.016, "Brass", segments=5)
        p.span((s * 1.0 - 0.06, -0.012, 3.64), (s * 1.0 + 0.06, 0, 3.8), "Brass")
        p.cyl(0.11, 0.1, (s * 1.0, -0.46, 3.6), "Brass", radius2=0.04, segments=10)
        p.cyl(0.08, 0.012, (s * 1.0, -0.46, 3.548), "Sodium lamp", segments=10)
    _rust(p, [(r.uniform(-0.9, 0.9), z1 - 0.04) for _ in range(4)], -d, r, length=0.25)
    return p


def sign_charter_row(style: Style) -> Piece:
    """Charter Row's street sign: white enamel fingerplates edged in company
    navy on a cast navy post with a brass finial, kept clean: CHARTER ROW
    pointing +X, THE HULL and an arrow pointing -Y. Turn it to point the
    plates down the streets."""
    p = Piece("sign_charter_row", "prop", "2.9 m post, plates 1.1 m; collider the post", budget=SIGN)
    # The post: a cast foot, a fluted shaft, collars and a finial.
    p.lathe([(0.0, 0), (0.17, 0), (0.17, 0.06), (0.13, 0.1), (0.1, 0.28), (0.07, 0.32), (0.0, 0.32)], (0, 0, 0),
            "Charter navy", segments=12)
    p.cyl(0.05, 2.5, (0, 0, 1.55), "Charter navy", segments=10)
    for z in (0.34, 1.9, 2.62):
        p.cyl(0.065, 0.04, (0, 0, z), "Brass", segments=10)
    p.lathe([(0.0, 2.78), (0.05, 2.79), (0.07, 2.84), (0.04, 2.9), (0.0, 2.93)], (0, 0, 0), "Brass", segments=10)
    p.collider((0.2, 0.2, 2.8), (0, 0, 1.4))
    # The plates, off the post like a fingerpost: CHARTER ROW along +X (read
    # from ±Y), THE HULL along -Y (read from ±X), its arrow pointing that way.
    t = 0.024
    p.span((0.07, -t / 2, 2.28), (1.17, t / 2, 2.58), "Enamel white")
    p.span((-t / 2, -1.17, 1.94), (t / 2, -0.07, 2.24), "Enamel white")
    with p.painted():
        for facing, sy in (("-Y", -1), ("+Y", 1)):
            y = sy * (t / 2 + L1)
            for a, b in (((0.1, 2.55), (1.14, 2.565)), ((0.1, 2.295), (1.14, 2.31)),
                         ((0.1, 2.31), (0.115, 2.55)), ((1.125, 2.31), (1.14, 2.55))):
                p.span((a[0], y - COAT / 2, a[1]), (b[0], y + COAT / 2, b[1]), "Charter navy")
            p.lettering("CHARTER ROW", (0.62, y, 2.43), 0.085, "Charter navy", facing=facing, depth=COAT, spacing=1.05)
        for facing, sx in (("-X", -1), ("+X", 1)):
            x = sx * (t / 2 + L1)
            for a, b in (((-1.14, 2.21), (-0.1, 2.225)), ((-1.14, 1.955), (-0.1, 1.97))):
                p.span((x - COAT / 2, a[0], a[1]), (x + COAT / 2, b[0], b[1]), "Charter navy")
            p.lettering("THE HULL", (x, -0.5, 2.09), 0.08, "Charter navy", facing=facing, depth=COAT)
            arrow = [(-(b - 0.5) * 0.13, a * 0.13) for a, b in ARROW]
            p.prism(arrow, COAT, (x, -1.0, 2.09), "Charter navy", rot=(0, 0, 90))
    # The clamps holding them to the post.
    for z in (2.33, 2.53):
        p.span((0.0, -0.02, z - 0.02), (0.1, 0.02, z + 0.02), "Charter navy")
    for z in (1.99, 2.19):
        p.span((-0.02, -0.1, z - 0.02), (0.02, 0.0, z + 0.02), "Charter navy")
    return p


def sign_the_pads(style: Style) -> Piece:
    """The gantry over the Pads' yard entrance: two braced box legs, a beam
    in hazard paint with beacons on it, and a dark panel with THE PADS in
    raised yellow and the landing warning under it. A 6 m clear way between
    its legs, 3.7 m under the panel."""
    p = Piece("sign_the_pads", "prop", "7 m gantry, 5.45 m high, 6 m between its legs; collider the legs", budget=SIGN)
    r = rng(style, p.name)
    x, top = 3.2, 5.45
    for s in (-1, 1):
        p.span((s * x - 0.17, -0.17, 0.02), (s * x + 0.17, 0.17, top - 0.45), "Crew grey")
        p.span((s * x - 0.3, -0.3, 0), (s * x + 0.3, 0.3, 0.03), "Hull dark")
        _bolts(p, [(s * x + dx, 0.015) for dx in (-0.22, 0.22)], -0.3)
        _stripes(p, s * x - 0.17, 0.05, s * x + 0.17, 1.2, -0.17, width=0.07)
        with p.plain():
            for z0 in (0.6, 2.0, 3.4):
                p.tube([(s * x, 0.17, z0), (s * x, 0.4, z0 + 0.7), (s * x, 0.17, z0 + 1.4)], 0.022, "Crew grey", segments=4)
            p.span((s * x - 0.05, 0.17, 0.3), (s * x + 0.05, 0.42, 4.95), "Hull dark")
        p.collider((0.35, 0.35, top - 0.45), (s * x, 0, (top - 0.45) / 2))
    # The beam, striped, with its beacons.
    p.span((-x - 0.3, -0.2, top - 0.45), (x + 0.3, 0.2, top), "Hazard yellow")
    _stripes(p, -x - 0.3, top - 0.45, x + 0.3, top, -0.2, width=0.18)
    for bx in (-x, 0, x):
        p.cyl(0.08, 0.06, (bx, 0, top + 0.03), "Hull dark", segments=8)
        p.cyl(0.06, 0.12, (bx, 0, top + 0.12), "Sodium lamp", segments=8)
    # The panel, hung off the beam.
    z0, z1 = 3.75, 4.85
    p.span((-2.1, -0.06, z0), (2.1, 0.06, z1), "Hull dark")
    p.span((-2.16, -0.09, z0 - 0.05), (2.16, -0.06, z0 + 0.02), "Hazard yellow")
    p.span((-2.16, -0.09, z1 - 0.02), (2.16, -0.06, z1 + 0.05), "Hazard yellow")
    for s in (-1, 1):
        p.span((s * 1.6 - 0.03, -0.03, z1), (s * 1.6 + 0.03, 0.03, top - 0.45), "Hull dark")
    _raised(p, "THE PADS", 0, 4.42, 0.46, -0.06, "Hazard yellow", depth=0.024, bold=0.02, spacing=1.1)
    _text(p, "DRIFTER LANDING  -  KEEP CLEAR", 0, 3.96, 0.09, -0.06, "Stencil white", spacing=1.2)
    _bolts(p, [(sx * 2.0, zz) for sx in (-1, 1) for zz in (z0 + 0.12, z1 - 0.12)], -0.06)
    _rust(p, [(sx * 2.0, z1 - 0.12) for sx in (-1, 1)], -0.06, r, length=0.3)
    return p


def sign_south_gate(style: Style) -> Piece:
    """The board beside the South gate: SOUTH GATE in raised white on a grey
    header, the red band warning MASKS ON PAST THIS POINT with a rebreather
    drawn beside it, and the gate check's three rules, on two posts."""
    p = Piece("sign_south_gate", "prop", "2.6 m board on posts, 3 m high; colliders the posts and board", budget=SIGN)
    r = rng(style, p.name)
    w, z0, z1, d = 1.2, 1.35, 2.8, 0.05
    for s in (-1, 1):
        p.span((s * 1.25 - 0.06, -0.06, 0.02), (s * 1.25 + 0.06, 0.06, 3.0), "Crew grey")
        p.span((s * 1.25 - 0.16, -0.16, 0), (s * 1.25 + 0.16, 0.16, 0.04), "Hull dark")
        p.collider((0.14, 0.14, 3.0), (s * 1.25, 0, 1.5))
    p.collider((2.4, 0.12, z1 - z0), (0, 0, (z0 + z1) / 2))
    p.span((-w, -d, z0), (w, 0.0, z1), "Enamel white")
    p.span((-w - 0.02, -d - 0.02, 2.42), (w + 0.02, -d + 0.01, z1 + 0.02), "Crew grey")
    p.span((-w - 0.02, -d - 0.02, z0 - 0.02), (w + 0.02, -d + 0.01, z0 + 0.14), "Hazard yellow")
    _stripes(p, -w - 0.02, z0 - 0.02, w + 0.02, z0 + 0.14, -d - 0.02, width=0.07)
    _raised(p, "SOUTH GATE", 0, 2.61, 0.22, -d - 0.02, "Stencil white", depth=0.022, bold=0.02, spacing=1.1)
    with p.painted():
        # The red band and its warning.
        p.span((-w + 0.04, -d - L1 - COAT / 2, 2.12), (w - 0.04, -d - L1 + COAT / 2, 2.36), "Medical red")
        # A rebreather drawn on the band: the cup and two cans.
        p.cyl(0.07, COAT, (-w + 0.22, -d - L2, 2.25), "Stencil white", rot=(90, 0, 0), segments=8)
        for s in (-1, 1):
            p.cyl(0.035, COAT, (-w + 0.22 + s * 0.085, -d - L2, 2.19), "Stencil white", rot=(90, 0, 0), segments=6)
        p.lettering("MASKS ON PAST THIS POINT", (0.12, -d - L2, 2.24), 0.075, "Stencil white", depth=COAT, spacing=1.05)
    rules = ["GATE CHECK", "1  SHOW YOUR FILINGS", "2  MASK ON AND SEALED", "3  DECLARE ALL FILTERS"]
    for i, line in enumerate(rules):
        _text(p, line, -w + 0.12, 1.98 - i * 0.13, 0.075 if i else 0.09, -d, "Ink", align="LEFT",
              bold=0.01 if i == 0 else 0.0)
    _text(p, "BY ORDER OF THE CREW AND THE REGISTRY", -w + 0.12, 1.52, 0.04, -d, "Hull dark", align="LEFT")
    _bolts(p, [(sx * (w - 0.05), zz) for sx in (-1, 1) for zz in (z0 + 0.2, 2.3, 2.7)], -d)
    _rust(p, [(sx * (w - 0.05), 2.3) for sx in (-1, 1)], -d, r)
    return p


def sign_hull(style: Style) -> Piece:
    """The Hull's deck directory: THE HULL in raised Crew orange, and the
    decks one to three with what's on each and which way, YOU ARE ON DECK 2
    by a red dot. Wall-mounted: back at y = 0, the board 1.2–2.6 m up."""
    p = Piece("sign_hull", "prop", "1.6 × 1.4 m directory board; wall-mounted, no collider", budget=SIGN)
    r = rng(style, p.name)
    x, z0, z1, d = 0.8, 1.2, 2.6, 0.05
    p.span((-x, -d, z0), (x, 0, z1), "Hull dark")
    for a, b in (((-x - 0.04, -d - 0.025, z1 - 0.01), (x + 0.04, -0.01, z1 + 0.04)),
                 ((-x - 0.04, -d - 0.025, z0 - 0.04), (x + 0.04, -0.01, z0 + 0.01)),
                 ((-x - 0.04, -d - 0.025, z0 + 0.012), (-x + 0.01, -0.01, z1 - 0.012)),
                 ((x - 0.01, -d - 0.025, z0 + 0.012), (x + 0.04, -0.01, z1 - 0.012))):
        p.span(a, b, "Crew grey")
    _raised(p, "THE HULL", 0, 2.44, 0.15, -d, "Crew orange", depth=0.02, bold=0.02, spacing=1.15)
    rows = [("3", "THE STACKS", "ROOMS", "up"), ("2", "EXCHANGE", "MARKET", None), ("1", "LOWER DECKS", "AIR PLANT", "down")]
    for i, (n, a, b, way) in enumerate(rows):
        z = 2.12 - i * 0.3
        with p.painted():
            p.cyl(0.085, COAT, (-x + 0.17, -d - L1, z), "Crew orange", rot=(90, 0, 0), segments=12)
            p.span((-x + 0.06, -d - L1 - COAT / 2, z - 0.155), (x - 0.06, -d - L1 + COAT / 2, z - 0.148), "Crew grey")
            p.lettering(n, (-x + 0.17, -d - L2, z), 0.1, "Hull dark", depth=COAT)
        _text(p, a, -x + 0.32, z + 0.035, 0.07, -d, "Stencil white", align="LEFT")
        _text(p, b, -x + 0.32, z - 0.055, 0.045, -d, "Crew orange", align="LEFT")
        if way:
            _arrow(p, x - 0.17, z, 0.17, -d, "Crew orange", way)
    with p.painted():
        p.cyl(0.03, COAT, (-x + 0.17, -d - L1, 1.32), "Medical red", rot=(90, 0, 0), segments=8)
    _text(p, "YOU ARE ON DECK 2", -x + 0.26, 1.32, 0.045, -d, "Stencil white", align="LEFT")
    _bolts(p, [(sx * (x + 0.015), zz) for sx in (-1, 1) for zz in (z0 + 0.06, z1 - 0.06)], -d - 0.025)
    _rust(p, [(sx * (x + 0.015), z1 - 0.06) for sx in (-1, 1)], -d - 0.025, r)
    return p


# Deck direction plates ---------------------------------------------------------

def _wide(text: str, h: float) -> float:
    """About how wide text is at cap height h, in Blender's font."""
    return len(text) * h * 0.95


def _direction(name: str, note: str, style: Style, words, small, way, plate, ink) -> Piece:
    """A Crew direction plate: stencilled words and an arrow on a bolted
    plate at 2.05–2.42 m, sized to its words, a smaller plate under it if
    there's more to say."""
    p = Piece(name, "prop", note, budget=SIGN)
    r = rng(style, p.name)
    h, arrow = 0.11, 0.26
    width = 0.06 + _wide(words, h) + 0.08 + arrow + 0.08
    x, z0, z1, d = width / 2, 2.05, 2.42, 0.02
    _plate(p, -x, z0, x, z1, d, plate, r)
    _text(p, words, -x + 0.06, (z0 + z1) / 2, h, -d, ink, align="LEFT", bold=0.01, spacing=1.05)
    _arrow(p, x - 0.08 - arrow / 2, (z0 + z1) / 2, arrow, -d, ink, way)
    if small:
        sh = 0.05
        sw = 0.05 + _wide(small, sh) + 0.05
        _plate(p, -x, 1.86, -x + sw, 2.0, 0.015, "Crew grey", r)
        _text(p, small, -x + 0.05, 1.93, sh, -0.015, "Stencil white", align="LEFT")
    with p.painted():
        for _ in range(3):
            a, b = r.uniform(-x + 0.05, x - 0.1), r.uniform(z0 + 0.03, z1 - 0.06)
            p.prism([(0, 0), (0.05, 0.01), (0.04, 0.04), (0.005, 0.03)], COAT, (a, -d - L2, b), "Hull alloy")
    return p


def sign_lower_decks(style: Style) -> Piece:
    """LOWER DECKS and a down arrow, stencilled white on Crew orange, AIR
    PLANT on a grey plate under it: the way down to the Crew's decks."""
    return _direction("sign_lower_decks", "1.6 m Crew plate, 1.86–2.42 m up; wall-mounted", style,
                      "LOWER DECKS", "AIR PLANT - CREW ONLY", "down", "Crew orange", "Stencil white")


def sign_stacks(style: Style) -> Piece:
    """THE STACKS and an up arrow, orange on Crew grey: the way up to the
    rooms, where a new arrival's first bunk is."""
    return _direction("sign_stacks", "1.5 m Crew plate, 2.05–2.42 m up; wall-mounted", style,
                      "THE STACKS", None, "up", "Crew grey", "Crew orange")


def sign_exchange_arrow(style: Style) -> Piece:
    """EXCHANGE and a long arrow along the wall, white on Crew orange: the
    way to the floor. Turn it to point the other way."""
    return _direction("sign_exchange_arrow", "1.3 m Crew plate, 2.05–2.42 m up; wall-mounted", style,
                      "EXCHANGE", None, "right", "Crew orange", "Stencil white")


# Shop signs -------------------------------------------------------------------

def _icon(kind: str):
    """A shop's sign drawn as outlines (x across, z up, about 0.16 high)."""
    if kind == "filter":
        return [[(-0.05, -0.08), (0.05, -0.08), (0.05, 0.08), (-0.05, 0.08)],
                [(-0.06, 0.04), (0.06, 0.04), (0.06, 0.06), (-0.06, 0.06)]]
    if kind == "drop":
        return [[(0, 0.09)] + [(math.sin(a) * 0.055, -0.02 - math.cos(a) * 0.055 + 0.0)
                               for a in (math.pi * i / 8 for i in range(-6, 7))]]
    if kind == "gear":
        pts = []
        for i in range(16):
            a = math.tau * i / 16
            rr = 0.08 if i % 2 == 0 else 0.06
            pts.append((math.cos(a) * rr, math.sin(a) * rr))
        return [pts]
    if kind == "bowl":
        bowl = [(-0.09, 0.0), (0.09, 0.0)] + [(math.cos(a) * 0.09, -math.sin(a) * 0.07) for a in
                                               (math.pi * i / 8 for i in range(1, 8))]
        return [bowl, [(-0.03, 0.02), (-0.015, 0.02), (-0.01, 0.09), (-0.025, 0.09)],
                [(0.015, 0.02), (0.03, 0.02), (0.035, 0.09), (0.02, 0.09)]]
    if kind == "wrench":
        return [[(-0.012, -0.07), (0.012, -0.07), (0.012, 0.04), (0.035, 0.06), (0.02, 0.09), (0.008, 0.07),
                 (-0.008, 0.07), (-0.02, 0.09), (-0.035, 0.06), (-0.012, 0.04)]]
    if kind == "bed":
        return [[(-0.09, -0.04), (0.09, -0.04), (0.09, 0.0), (-0.09, 0.0)],
                [(-0.09, 0.0), (-0.07, 0.0), (-0.07, 0.06), (-0.09, 0.06)],
                [(-0.06, 0.0), (-0.02, 0.0), (-0.02, 0.025), (-0.06, 0.025)]]
    return []


def _blade(name: str, note: str, style: Style, words: str, sub: str, icon: str, board: str, ink: str) -> Piece:
    """A hand-painted shop sign hanging on two chains from a bracket off the
    wall: a patched board, painted both sides, its words, a line under them
    and what's sold drawn in. Back at y = 0, the board 2.25–2.85 m up."""
    p = Piece(name, "prop", note, budget=SIGN)
    r = rng(style, p.name)
    # The bracket: a wall plate, an arm out, a brace under it, a knob on the end.
    p.span((-0.07, -0.02, 2.82), (0.07, 0, 3.22), "Hull dark")
    _bolts(p, [(0, 2.88), (0, 3.16)], -0.02)
    p.span((-0.02, -0.98, 3.1), (0.02, -0.02, 3.15), "Hull dark")
    with p.plain():
        p.tube([(0, -0.02, 2.88), (0, -0.35, 3.0), (0, -0.62, 3.1)], 0.012, "Hull dark", segments=3)
        p.sphere(0.025, (0, -0.99, 3.125), "Hull dark", segments=4, rings=3)
        for y in (-0.26, -0.78):
            p.cable((0, y, 3.1), (0, y, 2.86), 0.005, "Hull alloy", steps=3, segments=3)
    # The board, cut a little out of true, painted on both faces.
    t = 0.03
    jag = [(-0.38, 0), (0.38, 0), (0.38 + r.uniform(-0.02, 0.01), 0.6), (-0.38 + r.uniform(-0.01, 0.02), 0.6)]
    p.prism([(a, b + 2.25) for a, b in jag], t, (0, -0.52, 0), board, rot=(0, 0, 90))
    p.span((-0.022, -0.92, 2.835), (0.022, -0.12, 2.86), "Hull dark")
    for facing, sx in (("-X", -1), ("+X", 1)):
        xo = sx * (t / 2 + L1)
        with p.painted():
            # Patches over old paint, then the words over them.
            # One patch in each half of the board, so none lie on another.
            for y_lo, y_hi in ((-0.86, -0.55), (-0.49, -0.18)):
                pw, ph = r.uniform(0.08, 0.16), r.uniform(0.06, 0.12)
                cy = r.uniform(y_lo + pw / 2, y_hi - pw / 2)
                cz = r.uniform(2.3, 2.78)
                p.span((xo - COAT / 2, cy - pw / 2, cz - ph / 2), (xo + COAT / 2, cy + pw / 2, cz + ph / 2), r.choice(REPAINTS))
        xo2 = sx * (t / 2 + L2)
        with p.painted():
            p.lettering(words, (xo2, -0.52, 2.69), min(0.1, 0.62 / _wide(words, 1.0)), ink, facing=facing,
                        depth=COAT, bold=0.008)
            p.lettering(sub, (xo2, -0.52, 2.56), 0.045, ink, facing=facing, depth=COAT)
            p.span((xo2 - COAT / 2, -0.75, 2.6), (xo2 + COAT / 2, -0.29, 2.605), ink)
            for outline in _icon(icon):
                # Seen from -X, +Y is on the left: drawn mirrored there.
                pts = [(-a * 0.9 if sx < 0 else a * 0.9, b * 0.9) for a, b in outline]
                p.prism(pts, COAT, (xo2, -0.52, 2.38), ink, rot=(0, 0, 90))
    return p


def shop_sign_filters(style: Style) -> Piece:
    """FILTERS, FAIR PRICE: a filter cartridge drawn under it."""
    return _blade("shop_sign_filters", "shop sign on a bracket; back at y = 0", style, "FILTERS", "FAIR PRICE",
                  "filter", "Repaint teal", "Stencil white")


def shop_sign_water(style: Style) -> Piece:
    """WATER, BY THE LITRE: a drop drawn under it, on cream."""
    return _blade("shop_sign_water", "shop sign on a bracket; back at y = 0", style, "WATER", "BY THE LITRE",
                  "drop", "Repaint cream", "Water blue")


def shop_sign_parts(style: Style) -> Piece:
    """PARTS, BUY AND SELL: a gear drawn under it, on Crew grey."""
    return _blade("shop_sign_parts", "shop sign on a bracket; back at y = 0", style, "PARTS", "BUY AND SELL",
                  "gear", "Crew grey", "Crew orange")


def shop_sign_noodles(style: Style) -> Piece:
    """NOODLES, HOT ALL HOURS: a steaming bowl drawn under it, on red."""
    return _blade("shop_sign_noodles", "shop sign on a bracket; back at y = 0", style, "NOODLES", "HOT ALL HOURS",
                  "bowl", "Repaint oxide", "Repaint cream")


def shop_sign_repairs(style: Style) -> Piece:
    """REPAIRS, ANY HOUR: a spanner drawn under it, on navy."""
    return _blade("shop_sign_repairs", "shop sign on a bracket; back at y = 0", style, "REPAIRS", "ANY HOUR",
                  "wrench", "Repaint navy", "Stencil white")


def shop_sign_rooms(style: Style) -> Piece:
    """ROOMS, ASK INSIDE: a bunk drawn under it, on green; the Stacks'."""
    return _blade("shop_sign_rooms", "shop sign on a bracket; back at y = 0", style, "ROOMS", "ASK INSIDE",
                  "bed", "Repaint green", "Repaint cream")


# Marking panels -----------------------------------------------------------------

def crew_panel(style: Style) -> Piece:
    """A Crew zone panel: DECK 2 over a big B-14, stencilled on orange with
    a black-and-yellow corner, bolted on at 1.6–2 m. Numbers on everything."""
    p = Piece("crew_panel", "prop", "0.6 × 0.42 m zone panel; wall-mounted", budget=SIGN)
    r = rng(style, p.name)
    x, z0, z1, d = 0.3, 1.6, 2.02, 0.02
    _plate(p, -x, z0, x, z1, d, "Crew orange", r)
    _text(p, "DECK 2", -x + 0.06, z1 - 0.07, 0.06, -d, "Hull dark", align="LEFT", bold=0.006)
    _text(p, "ZONE", x - 0.07, z1 - 0.07, 0.035, -d, "Hull dark", align="RIGHT")
    _text(p, "B-14", 0.03, 1.8, 0.13, -d, "Stencil white", bold=0.012, spacing=1.05)
    _stripes(p, -x + 0.01, z0 + 0.01, -x + 0.12, z0 + 0.11, -d, width=0.03)
    return p


def hazard_panel(style: Style) -> Piece:
    """A hazard panel: black and yellow stripes, DANGER, and AIR PLANT:
    AUTHORISED CREW ONLY, on yellow, 1.5–2 m up."""
    p = Piece("hazard_panel", "prop", "0.8 × 0.5 m hazard panel; wall-mounted", budget=SIGN)
    r = rng(style, p.name)
    x, z0, z1, d = 0.4, 1.5, 2.0, 0.02
    _plate(p, -x, z0, x, z1, d, "Hazard yellow", r)
    _stripes(p, -x + 0.02, z1 - 0.13, x - 0.02, z1 - 0.02, -d, width=0.05)
    with p.painted():
        tri = [(-0.05, 0), (0.05, 0), (0, 0.09)]
        p.prism(tri, COAT, (-x + 0.1, -d - L1, 1.7), "Hull dark")
    _text(p, "DANGER", 0.06, 1.75, 0.1, -d, "Hull dark", bold=0.01, spacing=1.1)
    _text(p, "AIR PLANT: AUTHORISED CREW ONLY", 0, 1.58, 0.023, -d, "Hull dark")
    return p


def notice_board_small(style: Style) -> Piece:
    """A small notice board in a wooden frame: filings pinned up, ruled,
    stamped, and one with a red header: COLLECTION FILED. Paperwork is how
    everything here is decided. The board 1.2–1.9 m up."""
    p = Piece("notice_board_small", "prop", "1 × 0.7 m notice board; wall-mounted", budget=SIGN)
    r = rng(style, p.name)
    x, z0, z1, d = 0.5, 1.2, 1.9, 0.03
    p.span((-x, -d, z0), (x, 0, z1), "Fabric sand")
    for a, b in (((-x - 0.04, -d - 0.02, z1), (x + 0.04, -0.005, z1 + 0.04)),
                 ((-x - 0.04, -d - 0.02, z0 - 0.04), (x + 0.04, -0.005, z0)),
                 ((-x - 0.04, -d - 0.02, z0), (-x, -0.005, z1)),
                 ((x, -d - 0.02, z0), (x + 0.04, -0.005, z1))):
        p.span(a, b, "Wood dark")
    sheets = [(-0.3, 1.6, 0.19, 0.25), (-0.06, 1.68, 0.17, 0.22), (0.2, 1.62, 0.2, 0.26), (-0.28, 1.34, 0.18, 0.2),
              (0.0, 1.38, 0.17, 0.22)]
    for i, (cx, cz, w, h) in enumerate(sheets):
        tilt = r.uniform(-5, 5)
        paper = "Paper" if i % 2 == 0 else "Paper aged"
        with p.painted():
            p.prism([(-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2)], COAT,
                    (cx, -d - L1, cz), paper, rot=(0, tilt, 0))
        with p.painted():
            for k in range(5):
                lw = w * r.uniform(0.5, 0.8)
                lz = h / 2 - 0.05 - k * 0.032
                p.prism([(-w / 2 + 0.02, lz), (-w / 2 + 0.02 + lw, lz), (-w / 2 + 0.02 + lw, lz + 0.008),
                         (-w / 2 + 0.02, lz + 0.008)], COAT, (cx, -d - L2, cz), "Ink", rot=(0, tilt, 0))
            if r.random() < 0.6:
                p.cyl(0.025, COAT, (cx + w * 0.22, -d - L2, cz - h * 0.3), "Medical red", rot=(90, 0, 0), segments=10)
    # The collection notice: paper, and the red words and ruled lines on it.
    cx, cz = 0.32, 1.36
    with p.painted():
        p.span((cx - 0.12, -d - L1 - COAT / 2, cz - 0.13), (cx + 0.12, -d - L1 + COAT / 2, cz + 0.13), "Paper")
    with p.painted():
        for k in range(4):
            p.span((cx - 0.1, -d - L2 - COAT / 2, cz - 0.1 + k * 0.035), (cx + 0.06, -d - L2 + COAT / 2, cz - 0.093 + k * 0.035), "Ink")
    _text(p, "COLLECTION", cx, cz + 0.095, 0.032, -d, "Medical red", bold=0.003, layer=L2)
    _text(p, "FILED", cx, cz + 0.045, 0.032, -d, "Medical red", bold=0.003, layer=L2)
    # Pins.
    with p.plain():
        for cx, cz, w, h in sheets + [(0.32, 1.36, 0.24, 0.26)]:
            p.sphere(0.008, (cx, -d - 0.018, cz + h / 2 - 0.02), r.choice(["Medical red", "Brass", "Status blue"]),
                     segments=4, rings=3)
    return p


# Fabric -------------------------------------------------------------------------

def _rug(p: Piece, style: Style, w, l, stripes_across: bool) -> Piece:
    """A rug w × l (along Y), lying with its top 1.4 cm up, faded, patterned
    with a border and either medallions or stripes, tasselled at its ends."""
    r = rng(style, p.name)
    base = r.choice(RUGS)
    others = [c for c in RUGS if c != base]
    top = 0.014
    hw, hl = w / 2, l / 2
    p.cloth([(-hw, -hl, top), (hw, -hl, top), (hw, hl, top), (-hw, hl, top)], base, sag=0.0, ripple=0.002,
            thickness=0.006, cell=0.2, seed=r.randint(0, 99), droop=(0, 0, 0, 0))
    z = top + 0.008 + L1  # well clear of the cloth's ripple
    a, b = r.sample(others, 2)
    with p.painted():
        bw = min(w, l) * 0.08
        for lo, hi in (((-hw + 0.04, -hl + 0.04), (hw - 0.04, -hl + 0.04 + bw)),
                       ((-hw + 0.04, hl - 0.04 - bw), (hw - 0.04, hl - 0.04)),
                       ((-hw + 0.04, -hl + 0.04 + bw), (-hw + 0.04 + bw, hl - 0.04 - bw)),
                       ((hw - 0.04 - bw, -hl + 0.04 + bw), (hw - 0.04, hl - 0.04 - bw))):
            p.span((lo[0], lo[1], z), (hi[0], hi[1], z + COAT), a)
        if stripes_across:
            n = int(l / 0.35)
            for i in range(n):
                y = -hl + 0.2 + (l - 0.4) * (i + 0.5) / n
                p.span((-hw + 0.12, y - 0.05, z), (hw - 0.12, y + 0.05, z + COAT), a if i % 2 else b)
        else:
            for y in (-l * 0.25, 0.0, l * 0.25):
                s = w * 0.22
                p.box((s, s, COAT), (0, y, z), b, rot=(0, 0, 45))
                p.box((s * 0.45, s * 0.45, COAT), (0, y, z + L1), a, rot=(0, 0, 45))
    with p.plain():
        for sy in (-1, 1):
            for i in range(int(w / 0.07)):
                x = -hw + 0.05 + i * 0.07
                p.tube([(x, sy * (hl + 0.006), 0.009), (x + r.uniform(-0.01, 0.01), sy * (hl + 0.07), 0.004)], 0.004, "Fabric sand",
                       segments=3)
    return p


def rug(style: Style) -> Piece:
    """A rug, 2 × 3 m, faded red or ochre or indigo with a border and three
    medallions: laid in a room in the Stacks or under a stall. Walked on."""
    return _rug(Piece("rug", "prop", "2 × 3 m rug, top 1.4 cm up; no collider", budget=FABRIC), style, 2.0, 3.0, False)


def rug_runner(style: Style) -> Piece:
    """A runner, 1 × 4 m, striped across: down a corridor, along a counter."""
    return _rug(Piece("rug_runner", "prop", "1 × 4 m runner, top 1.4 cm up; no collider", budget=FABRIC),
                style, 1.0, 4.0, True)


def _banner(name: str, note: str, style: Style, cloth: str, trim: str, design) -> Piece:
    """A faction banner on a bracket off the wall: a pole out from a wall
    plate at 3.5 m, braced, and the cloth hung flat from it with a weighted
    hem; its design painted on both faces. design(p, x, face)
    paints one face at x."""
    p = Piece(name, "prop", note, budget=FABRIC)
    p.span((-0.07, -0.02, 3.3), (0.07, 0, 3.62), "Hull dark")
    _bolts(p, [(0, 3.36), (0, 3.56)], -0.02)
    with p.plain():
        p.cyl(0.018, 1.0, (0, -0.52, 3.5), "Hull dark", rot=(90, 0, 0), segments=4)
        p.sphere(0.035, (0, -1.04, 3.5), "Brass", segments=5, rings=3)
        p.tube([(0, -0.02, 3.08), (0, -0.25, 3.3), (0, -0.4, 3.49)], 0.01, "Hull dark", segments=3)
        for y in (-0.16, -0.5, -0.86):
            p.torus(0.028, 0.006, (0, y, 3.5), "Hull alloy", rot=(0, 90, 0), segments=5, sides=3)
    # The cloth: in the YZ plane, hanging flat.
    y0, y1, top, bottom, t = -0.14, -0.9, 3.46, 1.72, 0.006

    def f(u, v):
        y = y0 + (y1 - y0) * u
        z = top - (top - bottom) * v
        return Vector((0.0, y, z))

    _sheet(p, f, 8, 14, t, cloth)
    p.tube([(0, y0 - 0.02, bottom - 0.01), (0, y1 + 0.02, bottom - 0.01)], 0.012, trim, segments=4)
    for face, sx in (("-X", -1), ("+X", 1)):
        x = -t / 2 - L1 if sx < 0 else t / 2 + L1
        design(p, x, face)
    return p


def _banner_paint(p: Piece, x, face, shapes):
    """Painted rectangles and lettering on a banner's face at x: shapes are
    ("rect", y0, z0, y1, z1, mat) or ("text", words, y, z, h, mat)."""
    with p.painted():
        for s in shapes:
            if s[0] == "rect":
                _, a, b, c, d, mat = s
                p.span((x - COAT / 2, min(a, c), b), (x + COAT / 2, max(a, c), d), mat)
            elif s[0] == "disc":
                _, y, z, rr, mat = s
                p.cyl(rr, COAT, (x, y, z), mat, rot=(0, 90, 0), segments=14)
            else:
                _, words, y, z, h, mat = s
                p.lettering(words, (x, y, z), h, mat, facing=face, depth=COAT)


def banner_charter(style: Style) -> Piece:
    """The Charter Families' banner: company white edged in navy, a navy
    band, CHARTER FAMILIES. Old company colours, kept clean."""
    def design(p, x, face):
        m = -0.52
        _banner_paint(p, x, face, [
            ("rect", -0.16, 3.38, -0.88, 3.42, "Fabric navy"), ("rect", -0.16, 1.78, -0.88, 1.82, "Fabric navy"),
            ("rect", -0.16, 1.82, -0.2, 3.38, "Fabric navy"), ("rect", -0.84, 1.82, -0.88, 3.38, "Fabric navy"),
            ("rect", -0.2, 2.5, -0.84, 2.7, "Fabric navy"),
            ("text", "CHARTER", m, 3.05, 0.1, "Fabric navy"), ("text", "FAMILIES", m, 2.9, 0.06, "Fabric navy"),
            ("text", "88", m, 2.15, 0.16, "Fabric navy")])
    return _banner("banner_charter", "faction banner on a wall bracket, 3.5 m up; back at y = 0", style,
                   "Fabric white", "Fabric navy", design)


def banner_crew(style: Style) -> Piece:
    """The Crew's banner: utility orange with grey bands, a big stencilled
    number and AIR KEPT, the Crew's claim: they keep everyone breathing."""
    def design(p, x, face):
        m = -0.52
        _banner_paint(p, x, face, [
            ("rect", -0.16, 3.2, -0.88, 3.36, "Fabric grey"), ("rect", -0.16, 1.9, -0.88, 2.06, "Fabric grey"),
            ("text", "CREW", m, 2.95, 0.12, "Fabric charcoal"), ("text", "07", m, 2.55, 0.32, "Fabric white"),
            ("text", "AIR KEPT", m, 2.22, 0.06, "Fabric charcoal")])
    return _banner("banner_crew", "faction banner on a wall bracket, 3.5 m up; back at y = 0", style,
                   "Fabric orange", "Fabric grey", design)


def banner_registrar(style: Style) -> Piece:
    """The Registrars' banner: plain dark cloth with the brass seal, and
    REGISTRY under it. Plain dark coats, brass at the collar."""
    def design(p, x, face):
        m = -0.52
        _banner_paint(p, x, face, [
            ("disc", m, 2.78, 0.22, "Fabric brass"),
            ("rect", -0.24, 2.02, -0.8, 2.05, "Fabric brass"), ("rect", -0.24, 1.88, -0.8, 1.91, "Fabric brass"),
            ("text", "REGISTRY", m, 2.24, 0.08, "Fabric brass")])
        # The seal's field a layer out from its disc, the book a layer out
        # from that.
        out = -1 if x < 0 else 1
        _banner_paint(p, x + out * (L2 - L1), face, [("disc", m, 2.78, 0.16, "Fabric charcoal")])
        _banner_paint(p, x + out * (L3 - L1), face, [("rect", m - 0.09, 2.71, m + 0.09, 2.86, "Fabric brass")])
    return _banner("banner_registrar", "faction banner on a wall bracket, 3.5 m up; back at y = 0", style,
                   "Fabric charcoal", "Fabric brass", design)


def street_awning(style: Style) -> Piece:
    """Canvas strung across a 6 m street: strips of the market's two
    fabrics sewn together, sagging between its ends, roped to rings in the
    walls each side. Shade, and somewhere to sell under. Its origin on the
    street's middle; its rings at x = ±3 m (the walls' faces), 3.95 m up."""
    p = Piece("street_awning", "prop", "6 m across a street, 3.5–3.95 m up; no collider", budget=FABRIC)
    main, stripe = FABRICS[style.fabric]
    hw, hd, top, sag = 2.6, 1.25, 3.8, 0.28
    n = 6

    def surface(u0, u1):
        def f(u, v):
            x = -hw + 2 * hw * (u0 + (u1 - u0) * u)
            y = -hd + 2 * hd * v
            z = top - sag * (1 - (x / hw) ** 2) - 0.06 * (1 - (2 * v - 1) ** 2)
            return Vector((x, y, z))
        return f

    for i in range(n):
        _sheet(p, surface(i / n, (i + 1) / n), 3, 8, 0.008, main if i % 2 == 0 else stripe)
    with p.plain():
        for sx in (-1, 1):
            for sy in (-1, 1):
                ring = (sx * 2.97, sy * hd, 3.95)
                p.torus(0.035, 0.007, ring, "Hull alloy", rot=(0, 90, 0), segments=5, sides=3)
                p.cable((sx * hw, sy * hd, top), (sx * 2.93, sy * hd, 3.95), 0.006, "Rope", sag=0.03, steps=3, segments=3)
            xs = sorted((sx * 3.0, sx * 2.98))
            p.span((xs[0], -hd - 0.08, 3.88), (xs[1], hd + 0.08, 4.02), "Hull dark")
    return p


def bunting(style: Style) -> Piece:
    """Eight metres of rag bunting: flags torn from old cloth in every colour
    going, knotted on a rope that sags across the street between eye bolts.
    Its origin on the street's middle; its ends at x = ±4 m, 3.8 m up."""
    p = Piece("bunting", "prop", "8 m string of rag flags, 3.3–3.8 m up; no collider", budget=FABRIC)
    r = rng(style, p.name)
    half, top, sag = 4.0, 3.8, 0.45

    def z(x):
        t = (x + half) / (2 * half)
        return top - sag * 4 * t * (1 - t)

    with p.plain():
        p.cable((-half, 0, top), (half, 0, top), 0.006, "Rope", sag=sag, steps=10, segments=3)
        for sx in (-1, 1):
            p.torus(0.03, 0.007, (sx * (half - 0.03), 0, top), "Hull alloy", rot=(0, 90, 0), segments=5, sides=3)
            xs = sorted((sx * half, sx * (half - 0.02)))
            p.span((xs[0], -0.06, top - 0.06), (xs[1], 0.06, top + 0.06), "Hull dark")
        colours = list(FABRICS[style.fabric]) + RUGS + ["Fabric white", "Fabric orange"]
        x = -half + 0.25
        while x < half - 0.2:
            w, h = r.uniform(0.18, 0.24), r.uniform(0.24, 0.32)
            rag = [(-w / 2, 0), (w / 2, 0), (r.uniform(-0.02, 0.02) + 0.01, -h), (r.uniform(-0.03, 0.0), -h * 0.9)]
            p.prism(rag, 0.003, (x, 0, z(x) - 0.006), r.choice(colours), rot=(r.uniform(-6, 6), 0, r.uniform(-18, 18)))
            x += r.uniform(0.3, 0.38)
    return p


def doorway_curtain(style: Style) -> Piece:
    """A curtain in a 1 × 2.1 m doorway: a rod across the head, one panel
    hanging, the other drawn aside and tied back. Privacy, in the Stacks,
    costs a length of cloth. Its origin on the floor in the doorway's middle;
    the rod's brackets at x = ±0.5 m (the jambs)."""
    p = Piece("doorway_curtain", "prop", "1 × 2.15 m doorway curtain; no collider", budget=FABRIC)
    r = rng(style, p.name)
    main, stripe = r.choice(list(FABRICS.values()))
    with p.plain():
        p.cyl(0.011, 0.95, (0, 0, 2.15), "Hull alloy", rot=(0, 90, 0), segments=4)
        for sx in (-1, 1):
            xs = sorted((sx * 0.5, sx * 0.48))
            p.span((xs[0], -0.03, 2.1), (xs[1], 0.03, 2.2), "Hull dark")
        for x in [-0.46 + i * 0.07 for i in range(7)] + [0.3 + i * 0.05 for i in range(4)]:
            p.torus(0.016, 0.004, (x, 0, 2.15), "Hull alloy", rot=(0, 90, 0), segments=5, sides=3)

    def hanging(x0, x1, folds, gather):
        def f(u, v):
            x = x0 + (x1 - x0) * u
            zz = 2.13 - 2.1 * v
            # Folds across it, deeper at the hem; drawn aside, pinched at the tie.
            y = 0.035 * math.sin(folds * math.pi * u) * (0.5 + 0.5 * v)
            if gather:
                pinch = math.exp(-((v - 0.55) / 0.12) ** 2)
                x = x + (0.47 - x) * 0.45 * pinch
            return Vector((x, y, zz))
        return f

    _sheet(p, hanging(-0.46, 0.0, 7, False), 14, 16, 0.006, main)
    _sheet(p, hanging(0.28, 0.48, 5, True), 8, 16, 0.006, main)
    with p.plain():
        p.torus(0.07, 0.012, (0.4, 0.0, 0.98), stripe, rot=(0, 0, 0), segments=6, sides=3)
    return p


PIECES = [sign_exchange, sign_charter_row, sign_the_pads, sign_south_gate, sign_hull,
          sign_lower_decks, sign_stacks, sign_exchange_arrow,
          shop_sign_filters, shop_sign_water, shop_sign_parts, shop_sign_noodles, shop_sign_repairs, shop_sign_rooms,
          crew_panel, hazard_panel, notice_board_small,
          rug, rug_runner, banner_charter, banner_crew, banner_registrar, street_awning, bunting, doorway_curtain]
