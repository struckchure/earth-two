"""The ground underfoot: the caravan road and the salvage haul track out on
the Fringe, the paved street from the South gate to the Hull, Charter Row's
flagstones and its garden lawns, the farms' tilled fields, and the pad the
drifter comes down on (docs/settlement.md).

The game has its own terrain (game/terrain.go): level in town and under the
roads and farms, dunes beyond, and it drops each piece onto the lowest
ground under it. So a ground piece is a thin slab that reads as a surface:
its top a couple of centimetres above its origin, a skirt below to hide the
gaps where the ground isn't quite flat, and no collider (the terrain carries
everyone). They're laid by the hundred, so the game draws each as a slab of
a dozen triangles with its ruts, stones, joints and paint baked on (kit's
Piece.lowpoly); what stands up out of it (a road's marker stakes, a lawn's
tufts) is drawn too.

Each tiles exactly at its size, its origin the middle of its footprint.
Roads run along Y (in at -Y, out at +Y); turn them to run along X."""
import math

from mathutils import Vector

from kit import PALETTE, Piece, Style, rng

CATEGORY = "Ground"

PALETTE.setdefault("Road dust", (0.44, 0.18, 0.085))
PALETTE.setdefault("Rut", (0.26, 0.10, 0.05))
PALETTE.setdefault("Tilled red", (0.30, 0.11, 0.05))
PALETTE.setdefault("Lawn", (0.08, 0.22, 0.04))
PALETTE.setdefault("Lawn light", (0.14, 0.32, 0.06))
PALETTE.setdefault("Pad concrete", (0.36, 0.34, 0.31))
PALETTE.setdefault("Pad paint", (0.75, 0.70, 0.55))
PALETTE.setdefault("Scorch", (0.22, 0.20, 0.19))
PALETTE.setdefault("Scorch light", (0.28, 0.26, 0.24))

TOP = 0.025    # a surface's top, above its origin
SKIRT = 0.10   # how far it reaches below, on level ground
DEEP = 0.30    # and on a road or track, which may cross a slope
ROAD = 6.0     # the caravan road's width
RUTS = 1.5     # its wheel ruts, either side of its middle


def _slab(p: Piece, w: float, d: float, mat: str, skirt: float = SKIRT, top: float = TOP) -> None:
    """The stand-in every ground piece is drawn as: a slab w across (X) and
    d along (Y), its top at top, reaching skirt below the origin."""
    with p.lowpoly():
        p.span((-w / 2, -d / 2, -skirt), (w / 2, d / 2, top), mat)


def _base(p: Piece, w: float, d: float, mat: str, skirt: float = SKIRT, top: float = 0.0) -> None:
    """The detailed model's body under its surface parts (top at top)."""
    p.span((-w / 2, -d / 2, -skirt), (w / 2, d / 2, top), mat)


def _stones(p: Piece, r, n: int, lo, hi, size=(0.03, 0.09)) -> None:
    """Loose stones, flattened, lying proud of the surface: baked on."""
    for _ in range(n):
        s = r.uniform(*size)
        at = (r.uniform(lo[0], hi[0]), r.uniform(lo[1], hi[1]), TOP + s * 0.15)
        p.sphere(s, at, r.choice(["Rock red", "Rock dark", "Rock red"]), scale=(1, r.uniform(0.6, 1), 0.4),
                 rot=(0, 0, r.uniform(0, 180)), segments=5, rings=3)


def _road_surface(p: Piece, w: float, d: float, ruts, r, rut_w: float = 0.42) -> None:
    """Packed earth from -w/2 to w/2 across and -d/2 to d/2 along, with wheel
    ruts at ruts (offsets across): the surface between them at TOP, the
    ruts worn down into it."""
    edges = sorted([-w / 2, w / 2] + [x + s * rut_w / 2 for x in ruts for s in (-1, 1)])
    for a, b in zip(edges, edges[1:]):
        in_rut = any(abs((a + b) / 2 - x) < rut_w / 2 for x in ruts)
        if in_rut:
            p.span((a, -d / 2, 0.0), (b, d / 2, 0.011), "Rut")
            # The hump of packed earth along the rut's bottom.
            p.span(((a + b) / 2 - 0.05, -d / 2, 0.0), ((a + b) / 2 + 0.05, d / 2, 0.015), "Rut")
        else:
            p.span((a, -d / 2, 0.0), (b, d / 2, TOP), "Road dust")


def _stake(p: Piece, at, r) -> None:
    """A marker stake at the road's edge: a length of angle iron, its top
    painted Crew orange long ago. Drawn, not baked: it stands up."""
    x, y = at
    h = 0.6
    p.span((x - 0.025, y - 0.025, -0.15), (x + 0.025, y + 0.025, h), "Rust")
    with p.painted():
        p.span((x - 0.031, y - 0.031, h - 0.14), (x + 0.031, y + 0.031, h - 0.02), "Crew orange")
    with p.lowpoly():
        p.span((x - 0.025, y - 0.025, -0.15), (x + 0.025, y + 0.025, h), "Rust")


def road_straight(style: Style) -> Piece:
    """4 m of the caravan road: red earth packed hard by haulers, two wheel
    ruts worn into it, stones kicked to its sides, and a marker stake at
    each edge so it can be found in a storm. Runs along Y."""
    p = Piece("road_straight", "kit", "6 × 4 m of road, along Y; top 2.5 cm up, skirt 30 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, ROAD, 4, "Road dust", DEEP)
    _road_surface(p, ROAD, 4, (-RUTS, RUTS), r)
    _stones(p, r, 26, (-2.9, -1.95), (-2.0, 1.95))
    _stones(p, r, 26, (2.0, -1.95), (2.9, 1.95))
    _stones(p, r, 10, (-0.9, -1.95), (0.9, 1.95), size=(0.02, 0.05))
    with p.painted():
        # Oil and a tyre's tread pressed in the dust.
        for y in (-1.2, 0.9):
            p.cyl(r.uniform(0.15, 0.3), 0.004, (r.uniform(-0.6, 0.6), y, TOP + 0.006), "Rut", segments=6)
    _slab(p, ROAD, 4, "Road dust", DEEP)
    _stake(p, (-2.88, 0.0), r)
    _stake(p, (2.88, 0.0), r)
    return p


def road_corner(style: Style) -> Piece:
    """A quarter turn of the caravan road, 6 × 6 m: in at -Y, out at +X, its
    ruts sweeping round the inside corner."""
    p = Piece("road_corner", "kit", "6 × 6 m turn, in at -Y, out at +X; skirt 30 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, ROAD, ROAD, "Road dust", DEEP)
    p.span((-3, -3, 0.0), (3, 3, TOP), "Road dust")
    # The ruts: arcs round the inside corner (+X, -Y), 1.5 and 4.5 m out,
    # meeting the straights' ruts at both edges.
    c = Vector((3, -3, 0))
    for radius in (3 - RUTS, 3 + RUTS):
        n = 10
        for i in range(n):
            a0 = math.pi / 2 + (math.pi / 2) * i / n
            a1 = math.pi / 2 + (math.pi / 2) * (i + 1) / n
            m = (a0 + a1) / 2
            seg = radius * (a1 - a0) + 0.02
            at = c + Vector((math.cos(m), math.sin(m), 0)) * radius
            with p.painted():
                p.box((0.42, seg, 0.005), (at.x, at.y, TOP + 0.004), "Rut", rot=(0, 0, math.degrees(m)))
    _stones(p, r, 24, (-2.9, 1.2), (2.9, 2.95))
    _stones(p, r, 14, (-2.95, -2.9), (-2.2, 2.9))
    _slab(p, ROAD, ROAD, "Road dust", DEEP)
    _stake(p, (-2.88, 2.88), r)
    return p


def road_junction(style: Style) -> Piece:
    """A T in the caravan road, 6 × 6 m: through along X, a branch off to -Y
    (where the South gate's road joins the caravan road)."""
    p = Piece("road_junction", "kit", "6 × 6 m T: through along X, branch to -Y; skirt 30 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, ROAD, ROAD, "Road dust", DEEP)
    p.span((-3, -3, 0.0), (3, 3, TOP), "Road dust")
    with p.painted():
        for y in (-RUTS, RUTS):
            p.span((-3, y - 0.21, TOP + 0.002), (3, y + 0.21, TOP + 0.006), "Rut")
        # The branch's ruts, running in to meet the through road's.
        for x in (-RUTS, RUTS):
            p.span((x - 0.21, -3, TOP + 0.002), (x + 0.21, -RUTS + 0.21, TOP + 0.006), "Rut")
    _stones(p, r, 24, (-2.9, 2.0), (2.9, 2.95))
    _stones(p, r, 10, (-2.9, -2.95), (-2.1, -2.2))
    _stones(p, r, 10, (2.1, -2.95), (2.9, -2.2))
    _slab(p, ROAD, ROAD, "Road dust", DEEP)
    _stake(p, (-2.88, 2.88), r)
    _stake(p, (2.88, 2.88), r)
    return p


def road_end(style: Style) -> Piece:
    """Where the caravan road gives out: in at -Y, its ruts fading and its
    edges breaking up into loose dust and stones by +Y."""
    p = Piece("road_end", "kit", "6 × 4 m, in at -Y, fading out by +Y; skirt 30 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, ROAD, 4, "Road dust", DEEP)
    p.span((-3, -2, 0.0), (3, 2, TOP), "Road dust")
    with p.painted():
        for x in (-RUTS, RUTS):
            n = 8
            for i in range(n):
                y0 = -2 + 3.4 * i / n
                w = 0.42 * (1 - i / n)
                p.span((x - w / 2, y0, TOP + 0.002), (x + w / 2, y0 + 3.4 / n, TOP + 0.006), "Rut")
        # Drifts of loose dust over its far end.
        for _ in range(12):
            p.cyl(r.uniform(0.3, 0.7), 0.004, (r.uniform(-2.6, 2.6), r.uniform(0.4, 1.9), TOP + 0.007), "Dust red",
                  segments=7)
    _stones(p, r, 40, (-2.9, -1.95), (2.9, 1.95))
    _slab(p, ROAD, 4, "Road dust", DEEP)
    _stake(p, (-2.88, -1.0), r)
    return p


def track_straight(style: Style) -> Piece:
    """4 m of the salvage haul track: narrower and rougher than the caravan
    road, rutted deep by crawlers, littered with stones and the odd scrap of
    metal fallen off a load."""
    p = Piece("track_straight", "kit", "3 × 4 m of track, along Y; skirt 30 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, 3, 4, "Road dust", DEEP)
    _road_surface(p, 3, 4, (-0.75, 0.75), r, rut_w=0.36)
    _stones(p, r, 40, (-1.45, -1.95), (1.45, 1.95), size=(0.03, 0.11))
    for _ in range(3):
        p.box((r.uniform(0.08, 0.2), r.uniform(0.05, 0.12), 0.006), (r.uniform(-1.2, 1.2), r.uniform(-1.8, 1.8), TOP + 0.004),
              r.choice(["Rust", "Hull alloy", "Repaint teal"]), rot=(0, 0, r.uniform(0, 180)))
    with p.painted():
        # A crawler's tread pressed into the dust in each rut.
        for x in (-0.75, 0.75):
            for i in range(10):
                p.span((x - 0.14, -1.9 + i * 0.4, TOP + 0.002), (x + 0.14, -1.8 + i * 0.4, TOP + 0.006), "Rut")
    _slab(p, 3, 4, "Road dust", DEEP)
    return p


def gate_street(style: Style) -> Piece:
    """2 × 2 m of the paved street from the South gate to the Hull: slabs of
    red-soil concrete poured when the company still paid for things, cracked
    and patched since, red dust packed into the joints, a drain grate."""
    p = Piece("gate_street", "kit", "2 × 2 m street paving; top 2.5 cm up, skirt 10 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, 2, 2, "Concrete")
    # Four slabs a little apart, the joints between them dust-filled.
    halves = ((-0.985, -0.015), (0.015, 0.985))
    for x0, x1 in halves:
        for y0, y1 in halves:
            p.span((x0, y0, 0.0), (x1, y1, TOP), r.choice(["Concrete", "Concrete", "Red concrete"]))
    # Dust packed into the joints, the tile's edges included (half a joint
    # each side, so laid together they make a whole one).
    for c in (-1.0, 0.0, 1.0):
        w = 0.015
        p.span((-1, max(-1, c - w), 0.0), (1, min(1, c + w), 0.018), "Dust red")
        p.span((max(-1, c - w), -1, 0.0), (min(1, c + w), 1, 0.018), "Dust red")
    with p.painted():
        # A patch of newer concrete, cracks, and a grate at one corner.
        p.span((0.15, 0.2, TOP + 0.002), (0.75, 0.7, TOP + 0.006), "Red concrete")
        for _ in range(3):
            x, y = r.uniform(-0.9, -0.1), r.uniform(-0.9, 0.9)
            a = r.uniform(0, 180)
            p.box((r.uniform(0.2, 0.5), 0.008, 0.004), (x, y, TOP + 0.004), "Rock dark", rot=(0, 0, a))
        p.span((0.55, -0.95, TOP + 0.002), (0.95, -0.55, TOP + 0.006), "Hull dark")
        for i in range(5):
            x = 0.6 + i * 0.075
            p.span((x, -0.92, TOP + 0.006), (x + 0.03, -0.58, TOP + 0.009), "Grating")
    _slab(p, 2, 2, "Concrete")
    return p


def charter_paving(style: Style) -> Piece:
    """2 × 2 m of Charter Row's flagstones: pale, square-cut, laid in courses
    and swept clean. The only straight lines in town."""
    p = Piece("charter_paving", "kit", "2 × 2 m flagstones; top 2.5 cm up, skirt 10 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, 2, 2, "Charter stone")
    # Courses of flags half a metre deep, the joints offset course by course.
    for row in range(4):
        y0 = -1 + row * 0.5
        off = 0.25 if row % 2 else 0.0
        xs = [-1.0] + [x for x in (-0.75 + off, -0.25 + off, 0.25 + off, 0.75 + off) if -1 < x < 1] + [1.0]
        for a, b in zip(xs, xs[1:]):
            p.span((a + 0.004, y0 + 0.004, 0.0), (b - 0.004, y0 + 0.496, TOP),
                   r.choice(["Charter white", "Charter stone", "Charter white"]))
    _slab(p, 2, 2, "Charter white")
    return p


def _blade(p: Piece, at, yaw: float, lean: float, h: float, w: float, mat: str) -> None:
    """One blade of grass: a single triangle, two-sided (a face each way),
    standing at at, leaning lean degrees toward yaw."""
    base = Vector(at)
    d = Vector((math.cos(math.radians(yaw)), math.sin(math.radians(yaw)), 0))
    side = Vector((-d.y, d.x, 0)) * (w / 2)
    tip = base + d * (h * math.sin(math.radians(lean))) + Vector((0, 0, h * math.cos(math.radians(lean))))
    a, b, c = base - side, base + side, tip
    va = [p.bm.verts.new(v) for v in (a, b, c)]
    vb = [p.bm.verts.new(v) for v in (a, b, c)]
    faces = [p.bm.faces.new(va), p.bm.faces.new((vb[0], vb[2], vb[1]))]
    p._faces(faces, mat, smooth=False)


def lawn(style: Style) -> Piece:
    """2 × 2 m of clipped lawn from Charter Row's glass gardens: short and
    dense, watered every day with water the Fringe pays filters for. The
    turf is baked onto the slab; a sparse scatter of blades stands up out of
    it to catch the light."""
    p = Piece("lawn", "kit", "2 × 2 m lawn; top 2.5 cm up, skirt 10 cm; no collider", budget=200)
    r = rng(style, p.name)
    # (Not "Soil": the finish roughens soil, and it would push up through.)
    _base(p, 2, 2, "Lawn", top=0.02)
    # The turf: dense short blades, baked.
    with p.plain():
        for _ in range(900):
            _blade(p, (r.uniform(-0.98, 0.98), r.uniform(-0.98, 0.98), 0.02), r.uniform(0, 360), r.uniform(0, 30),
                   r.uniform(0.02, 0.045), 0.012, r.choice(["Lawn", "Lawn light", "Lawn"]))
    _slab(p, 2, 2, "Lawn")
    # What the game draws standing up: a few dozen blades.
    with p.lowpoly(), p.plain():
        for _ in range(30):
            _blade(p, (r.uniform(-0.95, 0.95), r.uniform(-0.95, 0.95), TOP - 0.002), r.uniform(0, 360),
                   r.uniform(5, 25), r.uniform(0.04, 0.06), 0.014, "Lawn light")
    return p


def farm_furrows(style: Style) -> Piece:
    """4 × 4 m of tilled field: red soil turned into furrows a half metre
    apart, running along X, a few clods and stones turned up with it."""
    p = Piece("farm_furrows", "kit", "4 × 4 m of furrows along X; top 2.5 cm up, skirt 10 cm; no collider", budget=200)
    r = rng(style, p.name)
    _base(p, 4, 4, "Rut", top=-0.02)
    # Ridges: a rounded section every half metre, crest just above TOP, the
    # darker, damper soil showing in the furrows between them.
    for i in range(8):
        y = -2 + 0.25 + i * 0.5
        p.prism([(-0.19, -0.02), (0.19, -0.02), (0.12, TOP), (0.04, TOP + 0.015), (-0.05, TOP + 0.016), (-0.13, TOP)],
                4.0, (0, y, 0), "Tilled red", rot=(0, 0, 90))
    _stones(p, r, 22, (-1.9, -1.9), (1.9, 1.9), size=(0.025, 0.06))
    with p.painted():
        # Lighter crests where they've dried.
        for i in range(8):
            y = -2 + 0.25 + i * 0.5
            p.span((-2, y - 0.03, TOP + 0.021), (2, y + 0.03, TOP + 0.025), "Dust red")
    _slab(p, 4, 4, "Tilled red")
    return p


def farm_rows(style: Style) -> Piece:
    """A 4 m row of Fringe grain on a ridge of the tilled field, along X:
    lean stalks, ears and leaves (as fringe.py's crop beds), left plain by
    the finish. Laid on farm_furrows, a row to a ridge (they're half a metre
    apart, at y = ±0.25, ±0.75, ...)."""
    from fringe import _plant
    p = Piece("farm_rows", "prop", "4 m row of grain along X, on a furrow; no collider", budget=6000)
    r = rng(style, p.name)
    n = 15
    for i in range(n):
        x = -1.9 + 3.8 * (i + 0.5) / n + r.uniform(-0.05, 0.05)
        _plant(p, (x, r.uniform(-0.03, 0.03), TOP + 0.01), r, "grain")
    return p


def _floor_text(p: Piece, text: str, at, height: float, mat: str, turn: float = 0.0, bold: float = 0.0) -> None:
    """Words lying flat on the ground, read from -Y (turned turn degrees
    about Z): kit's lettering stands its letters up on a face."""
    import bpy
    from mathutils import Matrix
    cu = bpy.data.curves.new("floor text", "FONT")
    cu.body = text
    cu.size = height / 0.72
    cu.align_x, cu.align_y = "CENTER", "CENTER"
    cu.extrude = 0.002
    cu.offset = bold * height
    o = bpy.data.objects.new("floor text", cu)
    bpy.context.scene.collection.objects.link(o)
    mesh = bpy.data.meshes.new_from_object(o.evaluated_get(bpy.context.evaluated_depsgraph_get()))
    bpy.data.objects.remove(o)
    bpy.data.curves.remove(cu)
    mesh.transform(Matrix.Translation(Vector(at)) @ Matrix.Rotation(math.radians(turn), 4, "Z"))
    before = len(p.bm.faces)
    p.bm.from_mesh(mesh)
    bpy.data.meshes.remove(mesh)
    p.bm.faces.ensure_lookup_table()
    p._faces(p.bm.faces[before:], mat, smooth=False)


def drifter_pad(style: Style) -> Piece:
    """The pad the drifters come down on, 26 × 50 m, the Patience on it now:
    one slab of pad concrete sawn into bays, its paint scorched and worn
    under the engines but still legible. A touchdown box in its middle, the
    pad number at each end, hazard chevrons, and tie-down rings round its
    edge. The drifter (19 × 45 m) stands centred on it, nose to -Y."""
    p = Piece("drifter_pad", "kit", "26 × 50 m landing pad; top 3 cm up, skirt 10 cm; no collider", budget=200)
    r = rng(style, p.name)
    W, L, top = 26.0, 50.0, 0.03
    p.span((-W / 2, -L / 2, -SKIRT), (W / 2, L / 2, 0.0), "Pad concrete")
    # Bays of slab between sawn joints every 6.5 by 6.25 m.
    nx, ny = 4, 8
    for i in range(nx):
        for j in range(ny):
            x0, x1 = -W / 2 + W * i / nx, -W / 2 + W * (i + 1) / nx
            y0, y1 = -L / 2 + L * j / ny, -L / 2 + L * (j + 1) / ny
            p.span((x0 + 0.02, y0 + 0.02, 0.0), (x1 - 0.02, y1 - 0.02, top),
                   r.choice(["Pad concrete", "Pad concrete", "Concrete"]))
    # Tie-down rings round the edge: steel loops in recessed pockets.
    rings = []
    for y in [-L / 2 + 2 + k * (L - 4) / 7 for k in range(8)]:
        rings += [(-W / 2 + 1.0, y), (W / 2 - 1.0, y)]
    for x in (-W / 2 + 7, 0.0, W / 2 - 7):
        rings += [(x, -L / 2 + 1.0), (x, L / 2 - 1.0)]
    for x, y in rings:
        p.torus(0.11, 0.018, (x, y, top + 0.012), "Steel", segments=8, sides=4)
    with p.painted():
        z0, z1 = top + 0.003, top + 0.007
        # The touchdown box: a broad frame round the middle, corner brackets.
        bw, bl, t = 17.0, 34.0, 0.45
        for (a, b) in (((-bw / 2, -bl / 2), (bw / 2, -bl / 2 + t)), ((-bw / 2, bl / 2 - t), (bw / 2, bl / 2)),
                       ((-bw / 2, -bl / 2), (-bw / 2 + t, bl / 2)), ((bw / 2 - t, -bl / 2), (bw / 2, bl / 2))):
            p.span((a[0], a[1], z0), (b[0], b[1], z1), "Pad paint")
        # A ring in the middle, the touchdown point.
        n = 48
        for k in range(n):
            ang = math.tau * (k + 0.5) / n
            p.box((0.5, 2 * math.pi * 7.0 / n + 0.03, 0.004), (math.cos(ang) * 7.0, math.sin(ang) * 7.0, z0 + 0.002),
                  "Pad paint", rot=(0, 0, math.degrees(ang)))
        # The pad's number at each end, read from the ends.
        _floor_text(p, "07", (0, -L / 2 + 5.0, z0 + 0.002), 4.5, "Pad paint", turn=0, bold=0.03)
        _floor_text(p, "07", (0, L / 2 - 5.0, z0 + 0.002), 4.5, "Pad paint", turn=180, bold=0.03)
        # Hazard chevrons across both ends.
        for end in (-1, 1):
            y = end * (L / 2 - 1.3)
            for k in range(9):
                x = -W / 2 + 2.4 + k * (W - 4.8) / 8
                for s in (-1, 1):
                    p.box((1.4, 0.38, 0.004), (x + s * 0.45, y, z0 + 0.002),
                          "Hazard yellow", rot=(0, 0, s * end * 35))
        # Engine scorch streaked back from under the drifter's engines (at
        # its +Y end), and the odd oil stain.
        for _ in range(22):
            x = r.choice((-5.5, 5.5)) + r.uniform(-1.8, 1.8)
            p.box((r.uniform(0.15, 0.5), r.uniform(2.5, 8), 0.004), (x, r.uniform(12, 21), z1 + 0.003),
                  r.choice(["Scorch", "Scorch light", "Scorch light"]), rot=(0, 0, r.uniform(-10, 10)))
        for _ in range(5):
            p.cyl(r.uniform(0.25, 0.7), 0.004, (r.uniform(-7, 7), r.uniform(-18, 10), z1 + 0.006), "Scorch light",
                  segments=8)
    _slab(p, W, L, "Pad concrete", top=top)
    return p


PIECES = [road_straight, road_corner, road_junction, road_end, track_straight, gate_street, charter_paving, lawn,
          farm_furrows, farm_rows, drifter_pad]
