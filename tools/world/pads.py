"""The Pads: the colony's landing pads, turned into freight yards. Drifters
land here, so new arrivals step off here, every player among them. Their
cargo comes off into containers and gets moved by cranes and loader gangs,
and the Quiet Book keeps its fronts among the stacks (docs/settlement.md,
docs/economy.md).

Like the rest of the world, each piece stands on its origin in metres with
its front looking along -Y. Pieces laid by the dozen (pad tiles, containers)
are low-poly stand-ins with their detail baked on (kit's Piece.lowpoly);
the yard's few big machines carry their detail as geometry."""
import math

from mathutils import Vector

from kit import PALETTE, REPAINTS, Piece, Style, rng

CATEGORY = "The Pads"

PALETTE.setdefault("Pad paint", (0.75, 0.70, 0.55))

CONTAINERS = ["Container blue", "Container green", "Container red", "Repaint teal", "Repaint oxide", "Repaint cream", "Container blue"]


def _beam(p: Piece, a, b, w: float, h: float, mat: str) -> None:
    """A square-section member from a to b, w across and h deep."""
    a, b = Vector(a), Vector(b)
    d = b - a
    q = d.normalized().to_track_quat("Z", "Y").to_euler()
    p.box((w, h, d.length), (a + b) / 2, mat, rot=tuple(math.degrees(r) for r in q))


def _hazard(p: Piece, lo, hi, along: str = "x", n: int = 0, slant: bool = False) -> None:
    """Hazard paint from lo to hi (a box a few millimetres proud of the face
    it's on): yellow and dark bands in turn along one axis. Paint, so it's
    baked in (see Piece.painted)."""
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


def _corrugated(p: Piece, x0: float, x1: float, z0: float, z1: float, y: float, side: int, mat: str,
                pitch: float = 0.28, depth: float = 0.035) -> None:
    """A corrugated steel sheet from x0 to x1 and z0 to z1, its back on the
    plane y and its ribs standing out towards side (-1 for -Y, +1 for +Y):
    trapezoidal ribs, as on a container's walls."""
    n = max(1, round((x1 - x0) / pitch))
    step = (x1 - x0) / n
    outline = []
    for i in range(n):
        x = x0 + i * step
        outline += [(x, 0.0), (x + 0.15 * step, depth), (x + 0.5 * step, depth), (x + 0.65 * step, 0.0)]
    outline += [(x1, 0.0), (x1, -0.008), (x0, -0.008)]
    p.prism(outline, z1 - z0, (0, y, (z0 + z1) / 2), mat, rot=(90 * -side, 0, 0))


def pad_tile(style: Style) -> Piece:
    """4 × 4 m of landing pad: poured red-soil concrete in four slabs with
    sawn joints, a painted edge line and dashes, a corner of hazard chevrons,
    a tie-down ring set in each slab, a drain, and oil and scorch where
    engines fired. Laid edge to edge, the lines and corners make up a pad's
    markings. The game draws a single slab (12 triangles) with all of that
    baked on. Like the deck, it has no collider: the level's ground is what's
    walked on."""
    p = Piece("pad_tile", "kit", "4 × 4 m pad, top at 0; walked on the level's ground")
    p.budget = 200
    r = rng(style, p.name)
    p.span((-2, -2, -0.3), (2, 2, -0.035), "Hull dark")
    # Four slabs, each a touch differently weathered, with 2 cm joints.
    for x0 in (-2, 0):
        for y0 in (-2, 0):
            mat = r.choice(["Concrete", "Concrete", "Red concrete"])
            p.span((x0 + 0.012, y0 + 0.012, -0.29), (x0 + 1.988, y0 + 1.988, 0), mat)
    with p.painted():
        # A tie-down ring in its pocket near each slab's middle (the ring
        # itself lies on the surface, below).
        rings = []
        for x0 in (-2, 0):
            for y0 in (-2, 0):
                cx, cy = x0 + 1.0 + r.uniform(-0.3, 0.3), y0 + 1.0 + r.uniform(-0.3, 0.3)
                if y0 < 0:
                    cy = min(cy, -1.0)
                p.cyl(0.11, 0.004, (cx, cy, 0.005), "Hull dark", segments=10)
                rings.append((cx, cy))
        # A drain slot by the front edge, its grating's bars.
        p.span((-0.6, -1.88, 0.003), (0.6, -1.76, 0.007), "Hull dark")
        for i in range(9):
            x = -0.55 + i * 0.1375
            p.span((x - 0.018, -1.88, 0.009), (x + 0.018, -1.76, 0.013), "Grating")
    for cx, cy in rings:
        p.torus(0.07, 0.012, (cx, cy, 0.02), "Steel", segments=10, sides=4)
        # The edge line along the front, and dashes across the middle.
        p.span((-1.98, -1.68, 0.003), (1.98, -1.52, 0.007), "Pad paint")
        for i in range(4):
            x = -1.75 + i
            p.span((x, -0.07, 0.003), (x + 0.5, 0.07, 0.007), "Pad paint")
        # Chevrons in the back corner: yellow and dark at 45°, the dark
        # painted over the yellow (laid 5 mm above it).
        for i in range(5):
            p.box((0.14, 1.1 - i * 0.2, 0.004), (1.35 - i * 0.14, 1.35 - i * 0.03, 0.005 if i % 2 == 0 else 0.011),
                  "Hazard yellow" if i % 2 == 0 else "Hull dark", rot=(0, 0, 45))
        # Oil stains and a scorch, clear of the rings, over the paint.
        for cx, cy, rad in ((-0.25, 0.35, 0.3), (1.6, -0.35, 0.25), (-1.6, -0.4, 0.22)):
            p.cyl(rad * r.uniform(0.85, 1.0), 0.004, (cx, cy, 0.011), "Rock dark", segments=12)
    with p.lowpoly():
        p.span((-2, -2, -0.3), (2, 2, 0), "Concrete")
    return p


# An ISO container: 20 ft long, 8 ft wide, 8 ft 6 in high.
CL, CW, CH = 6.1, 2.44, 2.6
POST = 0.16  # the corner posts' width


def _container_frame(p: Piece) -> None:
    """The steel frame every container hangs off: corner posts, rails top and
    bottom, and the castings the cranes and twist-locks grab."""
    x, y, h = CL / 2, CW / 2, CH
    for sx in (-1, 1):
        for sy in (-1, 1):
            # Corner post: an angle section, inset from the castings.
            px, py = sx * (x - POST / 2), sy * (y - POST / 2)
            p.span((px - POST / 2 + 0.005, py - POST / 2 + 0.005, 0.12), (px + POST / 2 - 0.005, py + POST / 2 - 0.005, h - 0.12), "Hull dark")
            for z in (0, h - 0.12):
                p.span((px - POST / 2 - 0.004, py - POST / 2 - 0.004, z), (px + POST / 2 + 0.004, py + POST / 2 + 0.004, z + 0.12), "Rust")
                # The casting's oval apertures, on its outer faces.
                p.span((px + sx * (POST / 2 + 0.002) - 0.01, py - 0.03, z + 0.035), (px + sx * (POST / 2 + 0.002) + 0.01, py + 0.03, z + 0.085), "Hull dark")
                p.span((px - 0.035, py + sy * (POST / 2 + 0.002) - 0.01, z + 0.035), (px + 0.035, py + sy * (POST / 2 + 0.002) + 0.01, z + 0.085), "Hull dark")
    # Side rails, top and bottom, and the end rails.
    for sy in (-1, 1):
        for z0, z1 in ((0.0, 0.14), (h - 0.12, h - 0.01)):
            p.span((-x + POST, sy * y - (0.09 if sy > 0 else 0), z0 + 0.004), (x - POST, sy * y + (0 if sy > 0 else 0.09), z1), "Hull dark")
    for sx in (-1, 1):
        for z0, z1 in ((0.0, 0.16), (h - 0.2, h - 0.01)):
            p.span((sx * x - (0.09 if sx > 0 else 0), -y + POST, z0 + 0.004), (sx * x + (0 if sx > 0 else 0.09), y - POST, z1), "Hull dark")
    # Forklift pockets in the bottom side rails.
    for sy in (-1, 1):
        for cx in (-1.0, 1.0):
            p.span((cx - 0.18, sy * (y + 0.002) - 0.01, 0.03), (cx + 0.18, sy * (y + 0.002) + 0.01, 0.12), "Rubber")


def _container_skin(p: Piece, colour: str, blind_end: bool = True) -> None:
    """The corrugated walls and roof, and the corrugated blind end at -X."""
    x, y, h = CL / 2, CW / 2, CH
    for sy in (-1, 1):
        _corrugated(p, -x + POST + 0.02, x - POST - 0.02, 0.15, h - 0.13, sy * (y - 0.045), sy, colour, pitch=0.28, depth=0.035)
    # The roof: shallow ribs across it.
    n = 22
    for i in range(n):
        xx = -x + 0.2 + (CL - 0.4) * (i + 0.5) / n
        p.span((xx - 0.07, -y + 0.1, h - 0.035), (xx + 0.07, y - 0.1, h - 0.012), colour)
    p.span((-x + 0.1, -y + 0.09, h - 0.06), (x - 0.1, y - 0.09, h - 0.035), colour)
    if blind_end:
        _corrugated_end(p, -x + 0.045, colour)


def _corrugated_end(p: Piece, x: float, colour: str) -> None:
    """A container's blind end, its ribs standing out to -X."""
    y = CW / 2
    n = 6
    for i in range(n):
        yy = -y + POST + 0.06 + (CW - 2 * POST - 0.12) * (i + 0.5) / n
        p.span((x - 0.035, yy - 0.07, 0.17), (x, yy + 0.07, CH - 0.21), colour)
    p.span((x, -y + POST, 0.16), (x + 0.02, y - POST, CH - 0.2), colour)


def _doors(p: Piece, colour: str, angle: float = 0.0) -> list:
    """The two doors at a container's +X end: ribbed panels, four locking
    bars with their cams and handles, and hinges. angle swings them open
    (degrees); 0 is shut. Returns each leaf's (centre, rotation) for colliders."""
    x, y, h = CL / 2, CW / 2, CH
    w, hh = y - POST + 0.005, h - 0.36
    leaves = []
    for side in (1, -1):
        # Shut, a leaf hangs in the end frame; swung right round, its hinge
        # (on its strap) sits proud of the side wall.
        hinge = Vector((x - 0.05, side * ((y - POST + 0.01) if angle < 180 else (y + 0.07)), 0))
        a = math.radians(angle)
        # Along the leaf from its hinge, and its outward face's normal.
        along = Vector((math.sin(a), -side * math.cos(a), 0))
        out = Vector((math.cos(a), side * math.sin(a), 0))
        rot = (0, 0, math.degrees(math.atan2(along.y, along.x)) - 90)

        def at(u, v, z=0.0):
            return hinge + along * u + out * v + Vector((0, 0, z))

        p.box((0.04, w, hh), at(w / 2, 0.0, 0.18 + hh / 2), colour, rot=rot)
        # Pressed ribs across the leaf.
        for k in range(5):
            p.box((0.02, w - 0.12, 0.09), at(w / 2, 0.028, 0.32 + k * (hh - 0.3) / 4), colour, rot=rot)
        # Two locking bars, their cams top and bottom, a handle each.
        for f in (0.22, 0.62):
            p.cyl(0.022, hh + 0.12, at(w * f, 0.07, 0.12 + hh / 2 + 0.06), "Steel", segments=4)
            for z in (0.16, hh + 0.14):
                p.box((0.06, 0.08, 0.05), at(w * f, 0.055, z), "Hull dark", rot=rot)
            p.box((0.03, 0.22, 0.04), at(w * f + 0.09, 0.09, 1.15), "Steel", rot=rot)
        for z in (0.45, h / 2, h - 0.45):
            p.box((0.05, 0.08, 0.12), at(0.02, 0.025, z), "Hull dark", rot=rot)
        leaves.append((at(w / 2, 0.0, 0.18 + hh / 2), rot, w, hh))
    return leaves


def _container_paint(p: Piece, r, colour: str, open_end: bool = False) -> None:
    """Numbers, the data plate, somebody else's paint and rust runs."""
    x, y = CL / 2, CW / 2
    f = y - 0.01  # the ribs' outer faces
    with p.painted():
        p.stencil(f"{r.randint(100, 999)}", (1.4, -f - 0.006, 2.05), 0.36, "Stencil white")
        p.stencil(f"{r.randint(1, 9)}0", (-1.0, f + 0.006, 1.9), 0.4, "Stencil white", facing="+Y")
        # A repainted patch, rust running down over it.
        patch = r.choice([c for c in REPAINTS if c != colour])
        p.span((-2.2, -f - 0.008, 0.7), (-1.1, -f - 0.004, 1.6), patch)
        for i in range(3):
            xx = r.uniform(-2.6, 2.4)
            p.span((xx, -f - 0.015, 0.3 + r.uniform(0, 0.6)), (xx + 0.05, -f - 0.011, 2.3 - r.uniform(0, 0.5)), "Rust")
        if not open_end:
            p.span((x - 0.024, -0.25, 1.9), (x - 0.02, 0.25, 2.2), "Stencil white")


def shipping_container(style: Style) -> Piece:
    """A 20-foot freight container, off a drifter or older than the colony:
    corrugated, dented, repainted, numbered, its doors at its +X end. The top
    is flat, so it's somewhere to climb to off a crate. The game draws its
    box and castings (a little over 100 triangles) with the corrugation,
    doors and paint baked on."""
    p = Piece("shipping_container", "prop", "6.1 × 2.44 × 2.6 m ISO container; doors at +X")
    p.budget = 300
    r = rng(style, p.name)
    colour = r.choice(CONTAINERS)
    p.collider((CL, CW, CH), (0, 0, CH / 2))
    _container_frame(p)
    _container_skin(p, colour)
    # The door end's frame, and the doors shut in it.
    p.span((CL / 2 - 0.09, -CW / 2 + POST, CH - 0.2), (CL / 2 - 0.005, CW / 2 - POST, CH - 0.01), "Hull dark")
    _doors(p, colour)
    _container_paint(p, r, colour)
    with p.lowpoly():
        x, y, h = CL / 2, CW / 2, CH
        p.span((-x + 0.005, -y + 0.005, 0.005), (x - 0.005, y - 0.005, h - 0.005), colour)
        for sx in (-1, 1):
            for sy in (-1, 1):
                px, py = sx * (x - POST / 2), sy * (y - POST / 2)
                for z in (0, h - 0.12):
                    p.span((px - POST / 2 - 0.004, py - POST / 2 - 0.004, z), (px + POST / 2 + 0.004, py + POST / 2 + 0.004, z + 0.12), "Rust")
    return p


def container_open(style: Style) -> Piece:
    """A container with its doors swung right round against its sides and
    nothing inside but dunnage: a room to walk into, a hiding place,
    somewhere a Quiet Book deal happens. The game draws its walls, floor,
    roof and doors as plain slabs with the detail baked on."""
    p = Piece("container_open", "prop", "6.1 × 2.44 × 2.6 m container, open at +X, walk-in")
    p.budget = 3000
    r = rng(style, p.name)
    colour = r.choice(CONTAINERS)
    x, y, h = CL / 2, CW / 2, CH
    _container_frame(p)
    _container_skin(p, colour)
    # Inside: the walls' inner faces, darker, the plywood floor on its cross
    # members, and dunnage.
    for sy in (-1, 1):
        p.span((-x + 0.05, sy * (y - 0.0675) - 0.0075, 0.16), (x - 0.03, sy * (y - 0.0675) + 0.0075, h - 0.12), "Hull dark")
    p.span((-x + 0.05, -y + 0.05, h - 0.115), (x - 0.03, y - 0.05, h - 0.09), "Hull dark")
    p.span((-x + 0.06, -y + 0.05, 0.03), (x - 0.02, y - 0.05, 0.15), "Wood")
    for i in range(12):
        xx = -x + 0.3 + i * (CL - 0.6) / 11
        p.span((xx - 0.008, -y + 0.06, 0.15), (xx + 0.008, y - 0.06, 0.156), "Wood dark")
    p.span((-2.6, -0.9, 0.16), (-1.4, 0.1, 0.28), "Wood")
    p.sphere(0.32, (-1.9, 0.75, 0.32), "Fabric sand", scale=(1.0, 0.7, 0.55), segments=10, rings=6)
    # The doors, swung right round against the sides.
    leaves = _doors(p, colour, angle=262)
    _container_paint(p, r, colour, open_end=True)
    p.collider((CL, CW, 0.15), (0, 0, 0.075))
    p.collider((CL, 0.12, h), (0, -y + 0.06, h / 2))
    p.collider((CL, 0.12, h), (0, y - 0.06, h / 2))
    p.collider((CL, CW, 0.14), (0, 0, h - 0.07))
    p.collider((0.12, CW, h), (-x + 0.06, 0, h / 2))
    for centre, rot, w, hh in leaves:
        p.collider((0.08, w, hh), tuple(centre), rot=rot)
    with p.lowpoly():
        p.span((-x, -y, 0), (x, y, 0.15), "Wood")
        p.span((-x, -y, h - 0.1), (x, y, h), colour)
        p.span((-x, -y + 0.06, 0.145), (-x + 0.06, y - 0.06, h - 0.1), colour)
        for sy in (-1, 1):
            p.span((-x, sy * y - (0.08 if sy > 0 else 0), 0.145), (x, sy * y + (0 if sy > 0 else 0.08), h - 0.1), colour)
        for centre, rot, w, hh in leaves:
            p.box((0.09, w, hh), centre, colour, rot=rot)
        p.span((-2.6, -0.9, 0.145), (-1.4, 0.1, 0.27), "Wood")
    return p


def gantry_crane(style: Style) -> Piece:
    """A gantry crane straddling a pad, 14 m between its legs and 10 m to
    its girders: what lifts containers out of a drifter's hold. Its legs and
    sill beams are the yard's own, Crew orange and hazard-banded; the bridge
    girder, trolley and hook on top came off an old factory crane. It runs on
    rails along Y."""
    p = Piece("gantry_crane", "prop", "14 m span, about 11 m high; legs at x ±7")
    p.budget = 60000
    span, top = 7.0, 9.6
    leg_y = 1.6
    for sx in (-1, 1):
        x = sx * span
        # The sill beam on its two bogies, each with two wheels on the rail.
        p.span((x - 0.35, -leg_y - 0.9, 0.35), (x + 0.35, leg_y + 0.9, 0.9), "Crew orange")
        for y in (-leg_y - 0.45, leg_y + 0.45):
            p.span((x - 0.3, y - 0.42, 0.06), (x + 0.3, y + 0.42, 0.36), "Hull dark")
            for dy in (-0.22, 0.22):
                p.cyl(0.17, 0.12, (x - 0.24, y + dy, 0.19), "Rust", rot=(0, 90, 0), segments=8)
                p.cyl(0.17, 0.12, (x + 0.24, y + dy, 0.19), "Rust", rot=(0, 90, 0), segments=8)
        # Rail stops at the beam's ends.
        for sy in (-1, 1):
            p.span((x - 0.25, sy * (leg_y + 0.9) - 0.04, 0.4), (x + 0.25, sy * (leg_y + 0.9) + 0.04, 0.85), "Rubber")
        _hazard(p, (x - 0.356, -leg_y - 0.9, 0.4), (x + 0.356, -leg_y - 0.4, 0.85), along="y", n=4)
        _hazard(p, (x - 0.356, leg_y + 0.4, 0.4), (x + 0.356, leg_y + 0.9, 0.85), along="y", n=4)
        p.collider((0.7, 2 * leg_y + 1.8, 0.9), (x, 0, 0.45))
        # The legs: two box girders a side with stiffener ribs, braced.
        for y in (-leg_y, leg_y):
            p.span((x - 0.3, y - 0.3, 0.9), (x + 0.3, y + 0.3, top), "Crew orange")
            for z in (2.0, 4.0, 6.0, 8.0):
                p.span((x - 0.32, y - 0.32, z), (x + 0.32, y + 0.32, z + 0.06), "Crew orange")
            _bolts(p, [(x + 0.31, y + dy, 1.0) for dy in (-0.2, 0, 0.2)], 0.025, 0.03, "Hull dark", rot=(0, 90, 0))
            p.collider((0.6, 0.6, top - 0.9), (x, y, (top + 0.9) / 2))
        for z0 in (2.5, 6.0):
            p.span((x - 0.12, -leg_y + 0.3, z0), (x + 0.12, leg_y - 0.3, z0 + 0.3), "Crew orange")
        _beam(p, (x, -leg_y + 0.3, 2.8), (x, leg_y - 0.3, 6.0), 0.14, 0.14, "Hull dark")
        _beam(p, (x, leg_y - 0.3, 2.8), (x, -leg_y + 0.3, 6.0), 0.14, 0.14, "Hull dark")
        # A ladder up the +Y leg's outer face, with a cage.
        lx = x + sx * 0.38
        for dy in (-0.2, 0.2):
            p.span((lx - 0.02, leg_y + dy - 0.02, 0.9), (lx + 0.02, leg_y + dy + 0.02, top), "Steel")
        for i in range(int((top - 1.2) / 0.3)):
            p.cyl(0.012, 0.4, (lx, leg_y, 1.2 + i * 0.3), "Steel", rot=(90, 0, 0), segments=2.4)
        for i in range(5):
            p.torus(0.36, 0.015, (lx + sx * 0.0, leg_y, 3.6 + i * 1.3), "Steel", rot=(0, 0, -90 * sx), segments=10, sides=2.4, arc=180)
        # The end tie on top of the legs, where the bridge rides.
        p.span((x - 0.5, -leg_y - 0.5, top), (x + 0.5, leg_y + 0.5, top + 0.55), "Hazard yellow")
        _hazard(p, (x - 0.5, -leg_y - 0.506, top + 0.05), (x + 0.5, -leg_y - 0.502, top + 0.5), along="x", n=5)
    # The bridge: an old factory crane's girder, trolley and hook, riding the
    # end ties, its hook hanging over the pad.
    p.source("overhead_crane", at=(0, 0, top + 0.55 - 5.2), length=2 * span + 1.0, res="2k", recolour="Hazard yellow")
    # The cab hung under the -X end, and a walkway along the front.
    p.span((-span + 0.9, -leg_y - 1.1, top - 2.3), (-span + 2.7, -leg_y + 0.3, top - 0.05), "Crew grey")
    p.span((-span + 0.95, -leg_y - 1.104, top - 1.6), (-span + 2.65, -leg_y - 1.1, top - 0.4), "Glass")
    p.span((-span + 0.85, -leg_y - 1.15, top - 2.36), (-span + 2.75, -leg_y + 0.35, top - 2.28), "Hull dark")
    p.span((-span + 0.5, -leg_y - 1.0, top + 0.5), (span - 0.5, -leg_y - 0.55, top + 0.55), "Grating")
    for i in range(10):
        xx = -span + 0.55 + i * (2 * span - 1.1) / 9
        p.span((xx - 0.025, -leg_y - 1.0, top + 0.55), (xx + 0.025, -leg_y - 0.95, top + 1.55), "Crew orange")
    p.tube([(-span + 0.55, -leg_y - 0.975, top + 1.55), (span - 0.55, -leg_y - 0.975, top + 1.55)], 0.03, "Crew orange", segments=4)
    # Festooned power cable along the walkway.
    for i in range(6):
        a = -span + 1.0 + i * 2.2
        p.cable((a, -leg_y - 0.6, top + 0.5), (a + 2.2, -leg_y - 0.6, top + 0.5), 0.025, "Rubber", sag=0.35, segments=3)
    with p.painted():
        p.stencil("3", (-span + 1.8, -leg_y - 1.11, top - 1.95), 0.5, "Stencil white")
    return p


def fuel_tank(style: Style) -> Piece:
    """A fuel tank on its saddles, for the drifters' landers and the yard's
    loaders: welded seams, a walkway and manway on top, a ladder up the end,
    valves and pipework, a hose on a reel beside it, and a couple of gas
    bottles somebody's left in its shade."""
    p = Piece("fuel_tank", "prop", "6.6 × 3.4 × 3.3 m tank on saddles, with a hose reel")
    p.budget = 60000
    r_tank, length, cz = 1.25, 5.2, 1.75
    shell = "Repaint cream"  # (not "Bleached": the finish treats that as cloth)
    p.cyl(r_tank, length, (0, 0, cz), shell, rot=(0, 90, 0), segments=20)
    for sx in (-1, 1):
        # Dished ends, a weld ring where they meet the shell.
        p.lathe([(r_tank, 0), (r_tank * 0.97, 0.18), (r_tank * 0.8, 0.42), (r_tank * 0.45, 0.58), (0, 0.63)],
                (sx * length / 2, 0, cz), shell, rot=(0, sx * 90, 0), segments=20)
        p.torus(r_tank, 0.018, (sx * length / 2, 0, cz), "Rust", rot=(0, 90, 0), segments=20, sides=2.4)
        # The saddles: a cradle of plate with stiffeners, bolted to a footing.
        p.prism([(-1.15, 0.12), (1.15, 0.12), (1.15, 0.72), (0.88, 1.08), (0.45, 0.72), (0, 0.52), (-0.45, 0.72), (-0.88, 1.08), (-1.15, 0.72)],
                0.24, (sx * 1.6, 0, 0), "Hull dark", rot=(0, 0, 90))
        for dy in (-0.75, 0, 0.75):
            p.span((sx * 1.6 - 0.25, dy - 0.02, 0.12), (sx * 1.6 + 0.25, dy + 0.02, 0.65), "Hull dark")
        p.span((sx * 1.6 - 0.35, -1.3, 0), (sx * 1.6 + 0.35, 1.3, 0.12), "Concrete")
        _bolts(p, [(sx * 1.6 + dx, dy, 0.13) for dx in (-0.25, 0.25) for dy in (-1.1, 1.1)], 0.035, 0.03, "Steel")
    # Circumferential seams.
    for x in (-1.3, 1.3):
        p.torus(r_tank, 0.015, (x, 0, cz), "Rust", rot=(0, 90, 0), segments=20, sides=2.4)
    # Manway with its bolt ring, a vent, a gauge.
    mz = cz + r_tank
    p.cyl(0.38, 0.22, (0.9, 0, mz), "Hull dark", segments=14)
    p.cyl(0.42, 0.04, (0.9, 0, mz + 0.11), "Hull dark", segments=14)
    _bolts(p, [(0.9 + 0.36 * math.cos(a), 0.36 * math.sin(a), mz + 0.14) for a in (k * math.tau / 10 for k in range(10))], 0.02, 0.03, "Steel")
    p.cyl(0.06, 0.4, (-0.4, 0.3, mz + 0.1), "Steel", segments=8)
    p.lathe([(0.11, 0), (0.11, 0.05), (0.04, 0.12), (0, 0.13)], (-0.4, 0.3, mz + 0.3), "Steel", segments=8)
    # A walkway along the top with a handrail.
    p.span((-2.2, -0.35, mz + 0.02), (0.4, 0.35, mz + 0.07), "Grating")
    for x in (-2.1, -1.1, -0.1):
        p.span((x - 0.02, 0.33, mz + 0.07), (x + 0.02, 0.37, mz + 1.05), "Crew orange")
    p.tube([(-2.1, 0.35, mz + 1.05), (-0.1, 0.35, mz + 1.05)], 0.025, "Crew orange", segments=4)
    # The ladder up the +X end.
    lx = length / 2 + 0.75
    for y in (-0.22, 0.22):
        p.span((lx - 0.03, y - 0.03, 0), (lx + 0.03, y + 0.03, mz + 0.9), "Crew orange")
    for i in range(10):
        p.cyl(0.016, 0.44, (lx, 0, 0.3 + i * 0.3), "Crew orange", rot=(90, 0, 0), segments=2.4)
    _beam(p, (lx, 0, mz - 0.2), (length / 2 + 0.3, 0, mz - 0.2), 0.08, 0.08, "Hull dark")
    # Valves and pipework out of the bottom, down to the hose reel.
    p.tube([(-1.0, -0.2, cz - r_tank + 0.05), (-1.0, -0.2, 0.55), (-1.0, -1.3, 0.55), (-0.45, -1.3, 0.55)], 0.06, "Steel", segments=8)
    p.cyl(0.11, 0.18, (-1.0, -0.7, 0.55), "Hull dark", rot=(90, 0, 0), segments=10)
    p.torus(0.14, 0.02, (-1.0, -0.82, 0.55), "Fabric red", rot=(90, 0, 0), segments=12, sides=2.4)
    # The hose reel, in front.
    p.span((-0.55, -1.95, 0), (0.55, -1.45, 0.08), "Hull dark")
    for x in (-0.45, 0.45):
        p.prism([(-0.22, 0), (0.22, 0), (0.05, 0.62), (-0.05, 0.62)], 0.05, (x, -1.7, 0.08), "Crew grey", rot=(0, 0, 90))
    p.cyl(0.44, 0.06, (-0.38, -1.7, 0.66), "Crew orange", rot=(0, 90, 0), segments=16)
    p.cyl(0.44, 0.06, (0.38, -1.7, 0.66), "Crew orange", rot=(0, 90, 0), segments=16)
    p.cyl(0.2, 0.72, (0, -1.7, 0.66), "Hull dark", rot=(0, 90, 0), segments=10)
    for i in range(5):
        p.torus(0.29, 0.055, (-0.26 + i * 0.13, -1.7, 0.66), "Rubber", rot=(0, 90, 0), segments=14, sides=2.4)
    p.cable((0, -1.4, 1.0), (-0.45, -1.3, 0.55), 0.045, "Rubber", sag=-0.15, segments=3, steps=8)
    p.cable((0.3, -2.0, 0.5), (1.6, -2.4, 0.05), 0.045, "Rubber", sag=0.05, segments=3, steps=8)
    with p.painted():
        for i in range(5):
            p.cyl(r_tank + 0.006, 0.24, (-0.48 + i * 0.24, 0, cz), "Hazard yellow" if i % 2 == 0 else "Hull dark", rot=(0, 90, 0), segments=20)
        p.stencil("07", (-1.9, -r_tank - 0.008, cz + 0.1), 0.45, "Hull dark")
        # The hazard placard: a diamond on the shell's side.
        p.box((0.36, 0.006, 0.36), (1.9, -r_tank - 0.006, cz), "Medical red", rot=(0, 45, 0))
    p.collider((length + 1.2, 2 * r_tank, cz + r_tank), (0, 0, (cz + r_tank) / 2))
    p.collider((1.1, 0.5, 1.1), (0, -1.7, 0.55))
    return p


def floodlight_tower(style: Style) -> Piece:
    """A floodlight tower on a trailer skid, its generator humming at its
    foot: a telescoping mast, four lamps on a bar aimed down at the pad, the
    cable spiralling up the mast, stabiliser legs down. The Pads work
    through the night when a drifter's in."""
    p = Piece("floodlight_tower", "prop", "2.6 × 1.4 m trailer, 8.7 m to the lamps")
    p.budget = 30000
    # The trailer: a chassis on two wheels, a drawbar, four stabiliser legs.
    p.span((-1.1, -0.55, 0.42), (1.1, 0.55, 0.56), "Hazard yellow")
    p.span((-1.15, -0.6, 0.36), (1.15, 0.6, 0.42), "Hull dark")
    for sy in (-1, 1):
        p.cyl(0.3, 0.18, (0, sy * 0.72, 0.3), "Rubber", rot=(90, 0, 0), segments=14)
        p.cyl(0.16, 0.19, (0, sy * 0.72, 0.3), "Steel", rot=(90, 0, 0), segments=8)
        p.span((-0.4, sy * 0.62 - 0.05, 0.55), (0.4, sy * 0.62 + 0.05, 0.62), "Hull dark")
    _beam(p, (-1.1, 0, 0.45), (-1.9, 0, 0.35), 0.1, 0.1, "Hull dark")
    p.torus(0.07, 0.015, (-1.95, 0, 0.35), "Steel", rot=(90, 0, 0), segments=8, sides=2.4)
    for sx in (-1, 1):
        for sy in (-1, 1):
            _beam(p, (sx * 0.95, sy * 0.5, 0.45), (sx * 1.3, sy * 0.85, 0.12), 0.07, 0.07, "Hazard yellow")
            p.cyl(0.11, 0.04, (sx * 1.3, sy * 0.85, 0.02), "Hull dark", segments=8)
    # The generator on the deck, and a toolbox of a cabinet beside the mast.
    p.source("portable_generator", at=(-0.4, 0, 0.56), length=0.9, res="1k", recolour="Crew orange")
    p.span((0.15, -0.5, 0.56), (0.55, 0.5, 1.3), "Hazard yellow")
    p.span((0.155, -0.505, 0.65), (0.545, -0.5, 1.2), "Hull dark")
    p.collider((2.2, 1.2, 1.3), (0, 0, 0.65))
    # The mast: four telescoping sections, each with its collar.
    mx = 0.8
    z, radius = 0.56, 0.12
    for k, h in enumerate((2.3, 2.1, 2.0, 1.9)):
        p.cyl(radius, h, (mx, 0, z + h / 2), "Steel" if k else "Hazard yellow", segments=10)
        p.cyl(radius + 0.02, 0.1, (mx, 0, z + h - 0.05), "Hull dark", segments=10)
        z += h - 0.1
        radius -= 0.022
    top = z
    p.collider((0.3, 0.3, top - 1.3), (mx, 0, (top + 1.3) / 2))
    # The cable, spiralling up the mast.
    pts = [(mx + 0.17 * math.cos(t * 0.9), 0.17 * math.sin(t * 0.9), 0.9 + t * (top - 1.2) / 24) for t in range(25)]
    p.tube(pts, 0.018, "Rubber", segments=3)
    # The lamp bar, and four lamps aimed out and down.
    p.span((mx - 1.0, -0.08, top), (mx + 1.0, 0.08, top + 0.12), "Hull dark")
    p.cyl(0.08, 0.2, (mx, 0, top + 0.06), "Hull dark", segments=8)
    for i, x in enumerate((-0.75, -0.25, 0.25, 0.75)):
        p.source("security_light", at=(mx + x, -0.05, top - 0.42), height=0.42, rot=(-20, 0, 0), res="1k")
    with p.painted():
        p.stencil("12", (0.35, -0.508, 1.05), 0.2, "Hull dark")
        _hazard(p, (-1.1, -0.556, 0.43), (1.1, -0.552, 0.55), along="x", n=11)
    return p


def cargo_net_pile(style: Style) -> Piece:
    """A pallet of cargo under a net, waiting for a hauler: a military crate
    on top of plastic crates, a sack and a drum, lashed down under a rope
    net hooked to the pallet's corners. Too high to vault; mantled."""
    p = Piece("cargo_net_pile", "prop", "1.3 × 1.1 × 1.3 m netted pallet: mantle")
    r = rng(style, p.name)
    # The pallet: deck boards on stringer blocks.
    for y in (-0.47, 0, 0.47):
        for x in (-0.55, 0, 0.55):
            p.span((x - 0.07, y - 0.06, 0), (x + 0.07, y + 0.06, 0.09), "Wood dark")
        p.span((-0.62, y - 0.05, 0.09), (0.62, y + 0.05, 0.11), "Wood dark")
    for i in range(7):
        x = -0.55 + i * 1.1 / 6
        p.span((x - 0.07, -0.53, 0.11), (x + 0.07, 0.53, 0.14), "Wood")
    # The load.
    p.source("plastic_crate_02", at=(-0.3, -0.25, 0.14), length=0.55, res="1k")
    p.source("plastic_crate_02", at=(-0.3, -0.25, 0.41), length=0.55, res="1k", recolour="Container blue")
    p.source("plastic_crate_02", at=(-0.3, 0.27, 0.14), length=0.55, res="1k", recolour="Crew orange")
    p.cyl(0.24, 0.52, (0.34, 0.22, 0.4), r.choice(["Rust", "Container blue", "Repaint teal"]), segments=14)
    for z in (0.3, 0.55):
        p.torus(0.245, 0.012, (0.34, 0.22, z), "Hull dark", segments=14, sides=2.4)
    p.sphere(0.3, (0.32, -0.27, 0.36), "Fabric sand", scale=(0.95, 0.75, 0.75), segments=10, rings=6)
    p.source("wooden_military_crate", at=(-0.05, 0.0, 0.69), length=1.15, res="1k")
    # The net: ropes over the top and down the sides, hooked at the corners.
    top = 1.18
    for x in (-0.48, -0.16, 0.16, 0.48):
        p.cable((x, -0.6, top), (x, 0.6, top), 0.012, "Canvas", sag=0.04, segments=3, steps=8)
        p.cable((x, -0.6, top), (x, -0.58, 0.14), 0.012, "Canvas", sag=0.0, segments=3, steps=2)
        p.cable((x, 0.6, top), (x, 0.58, 0.14), 0.012, "Canvas", sag=0.0, segments=3, steps=2)
    for y in (-0.36, 0, 0.36):
        p.cable((-0.68, y, top), (0.68, y, top), 0.012, "Canvas", sag=0.04, segments=3, steps=8)
        p.cable((-0.68, y, top), (-0.64, y, 0.14), 0.012, "Canvas", sag=0.0, segments=3, steps=2)
        p.cable((0.68, y, top), (0.64, y, 0.14), 0.012, "Canvas", sag=0.0, segments=3, steps=2)
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.torus(0.04, 0.01, (sx * 0.62, sy * 0.53, 0.12), "Steel", rot=(90, 0, 0), segments=8, sides=2.4)
    p.collider((1.24, 1.06, 1.25), (0, 0, 0.625))
    return p


def loader(style: Style) -> Piece:
    """A yard loader: a squat forklift that the loading gangs drive, forks
    out front (-Y), a rounded counterweight behind, an overhead guard over
    the seat, chains up the mast, a beacon, patched and numbered."""
    p = Piece("loader", "vehicle", "1.3 × 3.2 × 2.3 m forklift, forks to -Y")
    p.budget = 40000
    r = rng(style, p.name)
    w = 0.58
    paint = r.choice(["Crew orange", "Crew orange", "Hazard yellow"])
    # The body: sloped bonnet over the engine, steps, and the counterweight
    # behind, rounded off.
    p.prism([(-0.95, 0.28), (0.55, 0.28), (0.6, 0.78), (0.2, 0.92), (-0.75, 0.92), (-0.95, 0.75)], 2 * w, (0, 0, 0), paint, rot=(0, 0, 90))
    p.prism([(0.45, 0.25), (1.2, 0.25), (1.28, 0.45), (1.28, 0.95), (1.15, 1.12), (0.45, 1.12)], 2 * w + 0.04, (0, 0, 0), "Hull dark", rot=(0, 0, 90))
    for sx in (-1, 1):
        p.span((sx * (w + 0.005) - 0.004, -0.45, 0.42), (sx * (w + 0.005) + 0.004, 0.05, 0.75), "Grating")
        p.span((sx * w - (0.0 if sx > 0 else 0.12), -0.2, 0.25), (sx * w + (0.12 if sx > 0 else 0.0), 0.0, 0.3), "Grating")
    # Wheels: tyres with treads, hubs with nuts.
    for y, rr, wd in ((-0.62, 0.32, 0.26), (0.85, 0.27, 0.22)):
        for sx in (-1, 1):
            cx = sx * (w + 0.08)
            p.lathe([(0.0, -wd / 2), (rr * 0.55, -wd / 2), (rr * 0.92, -wd / 2), (rr, -wd / 2 + 0.03), (rr, wd / 2 - 0.03),
                     (rr * 0.92, wd / 2), (rr * 0.55, wd / 2), (0.0, wd / 2)], (cx, y, rr), "Rubber", rot=(0, 90, 0), segments=16)
            for k in range(10):
                a = k * math.tau / 10
                p.box((wd * 0.9, 0.03, 0.06), (cx, y + rr * math.cos(a), rr + rr * math.sin(a)), "Rubber", rot=(math.degrees(a), 0, 0))
            p.cyl(rr * 0.5, 0.04, (cx + sx * wd / 2, y, rr), "Steel", rot=(0, 90, 0), segments=10)
            _bolts(p, [(cx + sx * (wd / 2 + 0.02), y + 0.08 * math.cos(a), rr + 0.08 * math.sin(a)) for a in (k * math.tau / 5 for k in range(5))],
                   0.014, 0.02, "Hull dark", rot=(0, 90, 0))
    # The seat, the steering column and wheel, two levers.
    p.span((-0.28, 0.12, 0.92), (0.28, 0.52, 1.02), "Rubber")
    p.box((0.52, 0.1, 0.5), (0, 0.5, 1.25), "Rubber", rot=(-10, 0, 0))
    _beam(p, (0, -0.42, 0.92), (0, -0.32, 1.32), 0.07, 0.07, "Gunmetal")
    p.torus(0.17, 0.018, (0, -0.3, 1.35), "Rubber", rot=(55, 0, 0), segments=14, sides=2.4)
    for x in (0.18, 0.26):
        _beam(p, (x, -0.5, 0.92), (x + 0.02, -0.45, 1.25), 0.02, 0.02, "Steel")
        p.sphere(0.025, (x + 0.02, -0.45, 1.27), "Fabric red", segments=6, rings=4)
    # The overhead guard: four posts and a slatted roof.
    for sx in (-1, 1):
        _beam(p, (sx * 0.52, -0.55, 0.88), (sx * 0.5, -0.45, 2.2), 0.06, 0.06, "Hull dark")
        _beam(p, (sx * 0.52, 0.7, 1.12), (sx * 0.5, 0.62, 2.2), 0.06, 0.06, "Hull dark")
    p.span((-0.56, -0.52, 2.18), (0.56, 0.7, 2.24), "Hull dark")
    for i in range(5):
        p.span((-0.54, -0.42 + i * 0.25, 2.12), (0.54, -0.38 + i * 0.25, 2.18), "Hull alloy")
    # Beacon and work lights.
    p.cyl(0.07, 0.04, (0.35, 0.6, 2.26), "Hull dark", segments=8)
    p.lathe([(0.06, 0), (0.06, 0.08), (0.03, 0.12), (0, 0.13)], (0.35, 0.6, 2.28), "Sodium lamp", segments=8)
    for sx in (-1, 1):
        p.box((0.12, 0.08, 0.1), (sx * 0.45, -0.6, 2.1), "Hull dark", rot=(20, 0, 0))
        p.box((0.1, 0.01, 0.08), (sx * 0.45, -0.645, 2.09), "Sodium lamp", rot=(20, 0, 0))
    # The mast: two channels, the lift cylinder, chains, the carriage, forks.
    for sx in (-1, 1):
        p.span((sx * 0.36 - 0.06, -1.06, 0.18), (sx * 0.36 + 0.06, -0.94, 2.15), "Hull dark")
        p.span((sx * 0.36 - 0.03, -1.09, 0.2), (sx * 0.36 + 0.03, -1.06, 2.12), "Steel")
        for k in range(14):
            p.span((sx * 0.2 - 0.02, -1.0, 0.9 + k * 0.08), (sx * 0.2 + 0.02, -0.98, 0.95 + k * 0.08), "Gunmetal")
    p.span((-0.42, -1.0, 2.05), (0.42, -0.92, 2.15), "Hull dark")
    p.cyl(0.05, 1.5, (0, -0.96, 1.0), "Steel", segments=8)
    p.span((-0.45, -1.16, 0.3), (0.45, -1.08, 0.92), "Hull dark")
    for i in range(4):
        p.span((-0.44, -1.17, 0.36 + i * 0.15), (0.44, -1.162, 0.42 + i * 0.15), "Hull alloy")
    for sx in (-1, 1):
        p.span((sx * 0.25 - 0.05, -1.22, 0.12), (sx * 0.25 + 0.05, -1.14, 0.97), "Gunmetal")
        p.prism([(-1.15, 0.06), (-2.2, 0.06), (-2.25, 0.085), (-2.2, 0.11), (-1.15, 0.13)], 0.1, (sx * 0.25, 0, 0), "Gunmetal", rot=(0, 0, 90))
    with p.painted():
        _hazard(p, (-w - 0.02, 1.284, 0.45), (w + 0.02, 1.29, 0.95), along="x", n=7)
        p.stencil("14", (0, 1.288, 1.05), 0.12, "Stencil white", facing="+Y")
        for sx in (-1, 1):
            p.stencil(f"{r.randint(1, 9)}", (sx * (w + 0.006), 0.33, 0.7), 0.2, "Stencil white", facing="+X" if sx > 0 else "-X")
    p.collider((1.3, 2.15, 2.25), (0, -0.0, 1.125))
    return p


def quiet_book_shack(style: Style) -> Piece:
    """One of the Quiet Book's fronts: a lean-to of cut container panels with
    a corrugated roof, a roller shutter half up over a counter, cheap
    rebreathers and gas bottles for sale, a lamp, a hand-painted sign. What
    it really does is in the back."""
    p = Piece("quiet_book_shack", "prop", "3 × 2.5 × 2.8 m kiosk, counter 0.95 m: vaultable")
    p.budget = 20000
    r = rng(style, p.name)
    w, d, h = 1.5, 1.25, 2.6
    back, left, right = r.choice(CONTAINERS), r.choice(CONTAINERS), r.choice(CONTAINERS)
    # Walls: container panels, ribs out, framed in angle.
    _corrugated(p, -w + 0.06, w - 0.06, 0.05, h - 0.05, d - 0.04, 1, back, pitch=0.3)
    for sx, colour in ((-1, left), (1, right)):
        n = 8
        step = (2 * d - 0.12) / n
        outline = []
        for i in range(n):
            y = -d + 0.06 + i * step
            outline += [(y, 0.0), (y + 0.15 * step, 0.035), (y + 0.5 * step, 0.035), (y + 0.65 * step, 0.0)]
        outline += [(d - 0.06, 0.0), (d - 0.06, -0.008), (-d + 0.06, -0.008)]
        # The same profile, along Y on the side walls.
        pts = [(v * sx, y) for y, v in outline]
        p.prism([(y, v) for v, y in pts], h - 0.1, (sx * (w - 0.04), 0, (h - 0.1) / 2 + 0.05), colour, rot=(90, 0, 90))
        for y in (-d, d - 0.06):
            p.span((sx * w - (0.06 if sx > 0 else 0), y, 0), (sx * w + (0 if sx > 0 else 0.06), y + 0.06, h), "Hull dark")
    p.span((-w, d - 0.06, h - 0.08), (w, d, h), "Hull dark")
    p.collider((2 * w, 0.12, h), (0, d - 0.06, h / 2))
    p.collider((0.12, 2 * d, h), (-w + 0.06, 0, h / 2))
    p.collider((0.12, 2 * d, h), (w - 0.06, 0, h / 2))
    # The roof: corrugated sheet sloping back, on two purlins, overhanging.
    for y in (-d + 0.1, d - 0.1):
        p.span((-w - 0.1, y - 0.04, h - 0.02 + (y + d) * -0.06), (w + 0.1, y + 0.04, h + 0.06 + (y + d) * -0.06), "Hull dark")
    n = 13
    for i in range(n):
        x = -w - 0.15 + (2 * w + 0.3) * (i + 0.5) / n
        p.box((0.11, 2 * d + 0.6, 0.03), (x, -0.2, h + 0.12), "Rust", rot=(-3.5, 0, 0))
    p.box((2 * w + 0.3, 2 * d + 0.6, 0.02), (0, -0.2, h + 0.1), "Rust", rot=(-3.5, 0, 0))
    # The counter: a box of somebody's repainted panels, a steel top.
    p.span((-w + 0.06, -d, 0), (w - 0.06, -d + 0.4, 0.92), r.choice(REPAINTS))
    p.span((-w + 0.02, -d - 0.05, 0.92), (w - 0.02, -d + 0.45, 0.95), "Hull alloy")
    p.collider((2 * w - 0.12, 0.5, 0.95), (0, -d + 0.2, 0.475))
    # The shutter, rolled half up into its drum, slats and guides.
    p.cyl(0.16, 2 * w - 0.1, (0, -d + 0.08, h - 0.16), "Hull dark", rot=(0, 90, 0), segments=12)
    for i in range(7):
        z = h - 0.38 - i * 0.075
        p.span((-w + 0.07, -d + 0.04, z - 0.032), (w - 0.07, -d + 0.07, z + 0.032), "Steel" if i % 2 else "Hull alloy")
    for sx in (-1, 1):
        p.span((sx * (w - 0.08) - 0.03, -d + 0.02, 0.95), (sx * (w - 0.08) + 0.03, -d + 0.09, h - 0.3), "Hull dark")
    # Goods: gas bottles by the counter, masks and boxes on the shelves.
    p.source("small_lpg_tank", at=(w - 0.35, -d - 0.32, 0), height=0.62, res="1k")
    for z in (1.15, 1.7):
        p.span((-w + 0.1, d - 0.45, z - 0.04), (w - 0.1, d - 0.08, z), "Wood")
        for sx in (-1, 1):
            p.span((sx * (w - 0.12) - 0.02, d - 0.43, z - 0.3), (sx * (w - 0.12) + 0.02, d - 0.4, z), "Hull dark")
    for i, x in enumerate((-1.0, -0.55, -0.1)):
        _mask(p, (x, d - 0.25, 1.24))
    for i in range(3):
        p.span((0.25 + i * 0.32, d - 0.4, 1.15), (0.5 + i * 0.32, d - 0.12, 1.15 + 0.2 + 0.08 * (i % 2)), r.choice(["Paper aged", "Container green", "Crew grey"]))
    for i in range(6):
        p.lathe([(0.04, 0), (0.04, 0.16), (0.025, 0.19), (0, 0.2)], (-1.1 + i * 0.12, d - 0.22, 1.7), r.choice(["Glass", "Container green", "Crew orange"]), segments=8)
    for i in range(4):
        p.cyl(0.055, 0.2, (-0.9 + i * 0.14, -d + 0.2, 1.05), "Paper aged", segments=8)
        p.cyl(0.057, 0.04, (-0.9 + i * 0.14, -d + 0.2, 1.12), "Crew orange", segments=8)
    # The lamp on a bracket, its cable, and the sign on the roof's edge.
    _beam(p, (w - 0.2, -d + 0.05, h - 0.05), (w - 0.2, -d - 0.25, h - 0.15), 0.03, 0.03, "Hull dark")
    p.lathe([(0.0, 0.0), (0.09, 0.0), (0.12, 0.08), (0.05, 0.14), (0, 0.15)], (w - 0.2, -d - 0.28, h - 0.38), "Hull dark", segments=10)
    p.sphere(0.05, (w - 0.2, -d - 0.28, h - 0.36), "Sodium lamp", segments=8, rings=5)
    p.cable((w - 0.2, -d - 0.25, h - 0.15), (w - 0.06, -d + 0.2, h - 0.1), 0.01, "Rubber", sag=0.1, segments=3)
    p.span((-0.9, -d - 0.42, h + 0.2), (0.9, -d - 0.38, h + 0.62), "Wood dark")
    with p.painted():
        p.stencil("24", (-0.35, -d - 0.428, h + 0.41), 0.28, "Hazard yellow")
        p.span((0.1, -d - 0.428, h + 0.3), (0.75, -d - 0.424, h + 0.36), "Hazard yellow")
        p.span((0.1, -d - 0.428, h + 0.46), (0.6, -d - 0.424, h + 0.52), "Stencil white")
    return p


def _mask(p: Piece, at) -> None:
    """A half-mask rebreather, hung by its strap: the rubber cup, an exhale
    valve, two filter cans angled out on the cheeks, the strap."""
    x, y, z = at
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.075, 0.045), (0.085, 0.09), (0.06, 0.11), (0.0, 0.115)], (x, y - 0.05, z), "Rubber",
            rot=(90, 0, 0), segments=12)
    p.cyl(0.028, 0.03, (x, y - 0.17, z - 0.01), "Hull dark", rot=(90, 0, 0), segments=8)
    for sx in (-1, 1):
        p.cyl(0.032, 0.06, (x + sx * 0.08, y - 0.12, z - 0.02), "Crew orange", rot=(90, 0, sx * 30), segments=10)
        p.cyl(0.034, 0.012, (x + sx * 0.097, y - 0.15, z - 0.02), "Hull dark", rot=(90, 0, sx * 30), segments=10)
    p.torus(0.085, 0.006, (x, y - 0.02, z + 0.06), "Hull dark", rot=(70, 0, 0), segments=10, sides=2.4, arc=200)


def trade_table(style: Style) -> Piece:
    """A drifter-day trading table: an old painted table set up on the pad
    when a ship's in, under a canvas canopy, with off-world goods out on
    display (wine, medicine, a radio, seed trays, tinned food) and a brass
    scale, and more crates under it."""
    p = Piece("trade_table", "prop", "2 × 1.6 m table and canopy, table 0.8 m: vaultable")
    p.budget = 20000
    top = 0.8
    p.source("painted_wooden_table", at=(0, 0, 0), length=2.0, res="1k")
    p.collider((2.0, 0.92, top), (0, 0, top / 2))
    # The canopy on four poles, guyed, sagging.
    for sx in (-1, 1):
        for y, hh in ((-0.78, 2.1), (0.78, 2.35)):
            p.cyl(0.022, hh, (sx * 1.08, y, hh / 2), "Hull dark", segments=6)
            p.cable((sx * 1.08, y, hh), (sx * 1.55, y + (0.4 if y > 0 else -0.4), 0.02), 0.006, "Canvas", sag=0.03, segments=3, steps=4)
    p.cloth([(-1.16, -0.86, 2.12), (1.16, -0.86, 2.12), (1.16, 0.86, 2.37), (-1.16, 0.86, 2.37)], "Canvas", sag=0.09, ripple=0.012,
            droop=(0.4, 0.8, 0.2, 0.8), seed=3)
    p.box((2.32, 0.012, 0.2), (0, -0.875, 2.02), "Bleached")
    # Goods.
    p.source("wine_bottles_01", at=(-0.72, -0.15, top), length=0.5, res="1k")
    p.source("medical_box", at=(-0.1, -0.2, top), length=0.38, res="1k")
    p.source("vintage_radio_transceiver", at=(0.6, 0.12, top), length=0.42, res="1k")
    p.source("seeding_tray_01", at=(-0.35, 0.25, top), length=0.24, res="1k")
    p.source("russian_food_cans_01", at=(0.22, -0.25, top), height=0.12, res="1k")
    p.source("russian_food_cans_01", at=(0.33, -0.22, top), height=0.12, res="1k")
    # The scale: a post, a beam, two pans on chains.
    p.cyl(0.045, 0.02, (0.1, 0.32, top + 0.01), "Brass", segments=10)
    p.cyl(0.012, 0.36, (0.1, 0.32, top + 0.19), "Brass", segments=6)
    p.span((-0.12, 0.31, top + 0.36), (0.32, 0.33, top + 0.38), "Brass")
    for x in (-0.09, 0.29):
        p.cable((x, 0.32, top + 0.36), (x, 0.32, top + 0.2), 0.003, "Brass", segments=3, steps=2)
        p.lathe([(0.0, 0.0), (0.07, 0.0), (0.085, 0.02), (0.0, 0.02)], (x, 0.32, top + 0.18), "Brass", segments=12)
    # Crates under it.
    p.source("wooden_military_crate", at=(-0.4, 0.05, 0.0), length=1.0, res="1k")
    p.source("plastic_crate_02", at=(0.55, -0.05, 0.0), length=0.5, res="1k", recolour="Crew orange")
    return p


def pad_beacon(style: Style) -> Piece:
    """A pad's edge beacon: a short bollard of a post bolted down, an amber
    lamp in a wire guard on top, so landers find the pad through the dust."""
    p = Piece("pad_beacon", "prop", "1.1 m light post; no collider")
    p.budget = 8000
    p.cyl(0.17, 0.04, (0, 0, 0.02), "Hull dark", segments=12)
    _bolts(p, [(0.13 * math.cos(a), 0.13 * math.sin(a), 0.05) for a in (k * math.tau / 4 + 0.4 for k in range(4))], 0.016, 0.025, "Steel")
    p.lathe([(0.075, 0.0), (0.075, 0.06), (0.06, 0.1), (0.06, 0.82), (0.075, 0.85), (0.075, 0.9), (0.0, 0.9)], (0, 0, 0.04), "Hazard yellow", segments=12)
    p.lathe([(0.0, 0.0), (0.07, 0.0), (0.07, 0.1), (0.055, 0.15), (0.0, 0.17)], (0, 0, 0.94), "Sodium lamp", segments=12)
    for k in range(2):
        p.torus(0.12, 0.006, (0, 0, 1.0), "Hull dark", rot=(90, 0, k * 90 + 45), segments=10, sides=2.4, arc=180)
    p.torus(0.12, 0.008, (0, 0, 1.0), "Hull dark", segments=12, sides=2.4)
    with p.painted():
        for z in (0.25, 0.5):
            p.cyl(0.066, 0.12, (0, 0, z), "Hull dark", segments=12)
        p.stencil("4", (0, -0.068, 0.72), 0.08, "Hull dark")
    return p


def concrete_barrier(style: Style) -> Piece:
    """Jersey barriers: two 1.5 m sections of cast red-soil concrete laid end
    to end, chipped and stained, marking out the pads' lanes and stood behind
    when things go wrong. Low enough to vault."""
    p = Piece("concrete_barrier", "prop", "3 × 0.6 × 0.82 m barrier: vaultable")
    p.budget = 1500
    for x in (-0.75, 0.75):
        p.source("concrete_road_barrier", at=(x, 0, 0), length=1.49, res="1k", recolour="Red concrete")
    # The steel connector pinning the two sections together at the top.
    p.span((-0.06, -0.06, 0.74), (0.06, 0.06, 0.8), "Rust")
    p.cyl(0.02, 0.14, (0, 0, 0.8), "Steel", segments=4)
    p.collider((2.96, 0.6, 0.8), (0, 0, 0.4))
    return p


def bollard(style: Style) -> Piece:
    """A steel bollard filled with concrete, its cap domed, painted in hazard
    bands, scraped down to rust by a thousand loaders, bolted to a plate."""
    p = Piece("bollard", "prop", "0.25 m across, 1 m high")
    p.budget = 4000
    p.lathe([(0.11, 0.0), (0.11, 0.96), (0.095, 1.0), (0.05, 1.03), (0.0, 1.035)], (0, 0, 0.02), "Hazard yellow", segments=14)
    p.span((-0.15, -0.15, 0), (0.15, 0.15, 0.02), "Hull dark")
    _bolts(p, [(sx * 0.12, sy * 0.12, 0.03) for sx in (-1, 1) for sy in (-1, 1)], 0.014, 0.02, "Steel")
    with p.painted():
        for z in (0.55, 0.78):
            p.cyl(0.116, 0.12, (0, 0, z), "Hull dark", segments=14)
        p.cyl(0.117, 0.1, (0, 0, 0.08), "Rust", segments=14)
    p.collider((0.22, 0.22, 1.0), (0, 0, 0.5))
    return p


PIECES = [pad_tile, shipping_container, container_open, gantry_crane, fuel_tank, floodlight_tower,
          cargo_net_pile, loader, quiet_book_shack, trade_table, pad_beacon, concrete_barrier, bollard]
