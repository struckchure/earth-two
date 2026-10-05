"""The Hull kit: the pieces the Hull's decks are built from. The Hull is the
grounded colony ship Second Light, its decks cut open into streets (see
docs/settlement.md), so the kit is ship parts: deck plating, hull walls of
plates repainted many times, bulkheads, ribs, catwalks, stairs, ladders,
pipes and ducts.

Every piece sits on a 2 m grid with its origin at the middle of its
footprint, on the floor. A piece's front looks along -Y (the game's +Z).
The sizes that matter to traversal (character/traversal.go) are noted where
they're set.

Landfall lays these by the hundred, so most have a low-poly stand-in (see
kit.Piece.lowpoly): the game draws a few boxes, and the detailed model
(rivets, grating, welds, every plate) is baked onto them as texture."""
import math

from kit import DECK, GRID, REPAINTS, SLAB, Piece, Style, rng

CATEGORY = "The Hull"

H = GRID / 2
# Walls side by side overlap their colliders this much, so a ray or a sweep
# along a wall can't slip through the seam between two.
SEAM = 0.02


# Small parts ----------------------------------------------------------------

def _facing_rot(facing: str):
    """The turn that points a cylinder (made along Z) out of a face."""
    return {"-Y": (90, 0, 0), "+Y": (-90, 0, 0), "-X": (0, -90, 0), "+X": (0, 90, 0),
            "+Z": (0, 0, 0), "-Z": (180, 0, 0)}[facing]


def _bolt(p: Piece, at, facing: str, mat: str = "Steel", r: float = 0.012, proud: float = 0.006) -> None:
    """A bolt head (or a rivet) on a face, proud of it by proud and sunk into
    it as far: at is on the face."""
    p.cyl(r, 2 * proud, tuple(at), mat, rot=_facing_rot(facing), segments=4)


def _bolts_along(p: Piece, a, b, pitch: float, facing: str, mat: str = "Steel", r: float = 0.012,
                 proud: float = 0.006) -> None:
    """Bolts every pitch metres from a to b (both on the face)."""
    n = max(1, round(math.dist(a, b) / pitch))
    for i in range(n + 1):
        t = i / n
        _bolt(p, [a[k] + (b[k] - a[k]) * t for k in range(3)], facing, mat, r, proud)


def _louvres(p: Piece, x0: float, x1: float, z0: float, z1: float, y: float, side: int, mat: str, n: int) -> None:
    """A vent: a frame and n slats tilted down, on the face at y (side -1 for
    a face looking -Y)."""
    d = 0.025
    ya, yb = sorted((y, y + side * d))
    p.span((x0, ya, z0), (x1, yb, z0 + 0.02), mat)
    p.span((x0, ya, z1 - 0.02), (x1, yb, z1), mat)
    p.span((x0, ya, z0 + 0.02), (x0 + 0.02, yb, z1 - 0.02), mat)
    p.span((x1 - 0.02, ya, z0 + 0.02), (x1, yb, z1 - 0.02), mat)
    for i in range(n):
        z = z0 + 0.03 + (z1 - z0 - 0.06) * (i + 0.5) / n
        p.box((x1 - x0 - 0.04, 0.008, (z1 - z0 - 0.06) / n * 1.1), (0.5 * (x0 + x1), y + side * 0.012, z), mat,
              rot=(side * -40, 0, 0))


# Floors ---------------------------------------------------------------------

def deck_floor(style: Style) -> Piece:
    """2×2 m of deck: a slab with its top on the floor, two grating panels
    in a riveted frame, the dark of the slab and a cable run showing through
    the grating. It has no collider: a floor of boxes side by side has seams
    that catch a slide's sweep, so a level stands on one box under all its
    deck (the game's ground). The game draws a 12-triangle slab with all of
    this baked onto it."""
    p = Piece("deck_floor", "kit", "2 × 2 m deck plate, top at 0; walked on the level's ground")
    p.budget = 200
    r = rng(style, p.name)
    under = -0.045
    p.span((-H, -H, -SLAB), (H, H, under), "Hull dark")
    # The frame: the rim and a bar down the middle, bolted.
    f = 0.08
    for y in (-H, H - f):
        p.span((-H, y, under), (H, y + f, 0), "Hull alloy")
    for x in (-H, -f / 2, H - f):
        p.span((x, -H + f, under), (x + f, H - f, 0), "Hull alloy")
    for x in (-H + f / 2, 0, H - f / 2):
        _bolts_along(p, (x, -H + 0.12, 0), (x, H - 0.12, 0), 0.25, "+Z")
    for y in (-H + f / 2, H - f / 2):
        _bolts_along(p, (-H + 0.25, y, 0), (H - 0.25, y, 0), 0.25, "+Z")
    # Grating: bearing bars across each panel, cross rods every 15 cm.
    for x0, x1 in ((-H + f, -f / 2), (f / 2, H - f)):
        y = -H + f + 0.025
        while y < H - f - 0.02:
            p.span((x0, y - 0.006, -0.04), (x1, y + 0.006, -0.005), "Grating")
            y += 0.045
        x = x0 + 0.075
        while x < x1 - 0.05:
            p.span((x - 0.005, -H + f, -0.017), (x + 0.005, H - f, -0.005), "Grating")
            x += 0.15
    # Under the grating: a cable run or two.
    p.cable((-H + f, -0.35, under + 0.02), (H - f, -0.25, under + 0.02), 0.018, "Rubber")
    p.cable((-H + f, 0.42, under + 0.015), (H - f, 0.5, under + 0.015), 0.012, "Crew orange")
    # Now and then a plate welded over a broken panel.
    if r.random() < 0.5:
        x0 = r.choice((-H + f + 0.1, f / 2 + 0.1))
        y0 = r.uniform(-0.7, 0.2)
        p.span((x0, y0, -0.005), (x0 + 0.55, y0 + 0.45, 0.006), r.choice(REPAINTS))
        p.cable((x0 - 0.004, y0, 0.0), (x0 + 0.554, y0, 0.0), 0.006, "Rust")
        p.cable((x0 - 0.004, y0 + 0.45, 0.0), (x0 + 0.554, y0 + 0.45, 0.0), 0.006, "Rust")
    with p.lowpoly():
        p.span((-H, -H, -SLAB), (H, H, 0), "Hull dark")
    return p


def deck_slab(style: Style) -> Piece:
    """2×2 m of upper deck: a slab 0.3 m thick with its top on the floor,
    tread plate on top and the joists and conduits underneath (the ceiling
    of the deck below). Unlike deck_floor it has its own collider, for
    decks with nothing under them but air."""
    p = Piece("deck_slab", "kit", "2 × 2 × 0.3 m upper-deck slab, top at 0, walkable")
    p.budget = 200
    r = rng(style, p.name)
    p.collider((GRID, GRID, SLAB), (0, 0, -SLAB / 2))
    # The core, inside the faces the detail sits on.
    p.span((-H + 0.04, -H + 0.04, -0.25), (H - 0.04, H - 0.04, -0.02), "Hull dark")
    # Tread plate: four plates with a gap between them, a tread pressed into
    # each, rivets round their edges.
    g = 0.012
    for sx in (-1, 1):
        for sy in (-1, 1):
            x0, x1 = sorted((sx * g / 2, sx * (H - 0.04)))
            y0, y1 = sorted((sy * g / 2, sy * (H - 0.04)))
            mat = r.choice(REPAINTS) if r.random() < style.repaint * 0.5 else "Hull alloy"
            p.span((x0, y0, -0.02), (x1, y1, 0), mat)
            i = 0
            y = y0 + 0.05
            while y < y1 - 0.04:
                x = x0 + 0.05 + (0.04 if i % 2 else 0)
                while x < x1 - 0.04:
                    p.box((0.045, 0.012, 0.006), (x, y, 0.0), mat, rot=(0, 0, 45 if (i + round(x * 25)) % 2 else -45))
                    x += 0.08
                y += 0.045
                i += 1
            for a, b in (((x0 + 0.025, y0 + 0.025), (x1 - 0.025, y0 + 0.025)),
                         ((x0 + 0.025, y1 - 0.025), (x1 - 0.025, y1 - 0.025)),
                         ((x0 + 0.025, y0 + 0.025), (x0 + 0.025, y1 - 0.025)),
                         ((x1 - 0.025, y0 + 0.025), (x1 - 0.025, y1 - 0.025))):
                _bolts_along(p, (a[0], a[1], 0), (b[0], b[1], 0), 0.15, "+Z", mat, r=0.009, proud=0.004)
    # The edges: a channel round the slab, bolted.
    for y in (-H, H - 0.04):
        p.span((-H, y, -SLAB), (H, y + 0.04, 0), "Hull alloy")
    for x in (-H, H - 0.04):
        p.span((x, -H + 0.04, -SLAB), (x + 0.04, H - 0.04, 0), "Hull alloy")
    for y, facing in ((-H, "-Y"), (H, "+Y")):
        _bolts_along(p, (-H + 0.1, y, -0.15), (H - 0.1, y, -0.15), 0.2, facing)
    for x, facing in ((-H, "-X"), (H, "+X")):
        _bolts_along(p, (x, -H + 0.1, -0.15), (x, H - 0.1, -0.15), 0.2, facing)
    # Underneath: the skin, two joists across it, a conduit between them.
    p.span((-H + 0.04, -H + 0.04, -0.26), (H - 0.04, H - 0.04, -0.25), "Hull dark")
    for y in (-0.5, 0.5):
        p.span((-H + 0.04, y - 0.06, -0.3), (H - 0.04, y + 0.06, -0.285), "Crew grey")
        p.span((-H + 0.04, y - 0.008, -0.285), (H - 0.04, y + 0.008, -0.26), "Crew grey")
    p.cable((-H + 0.04, 0.05, -0.275), (H - 0.04, 0.05, -0.275), 0.02, "Crew orange")
    for x in (-0.6, 0.0, 0.6):
        p.span((x - 0.02, 0.075, -0.3), (x + 0.02, 0.11, -0.26), "Hull dark")
    with p.lowpoly():
        p.span((-H, -H, -SLAB), (H, H, 0), "Hull dark")
    return p


# Walls ----------------------------------------------------------------------

def _plates(p: Piece, style: Style, side: int, x0: float, x1: float, z0: float, z1: float, rows: int,
            skip=None, number: str | None = None) -> None:
    """Hull plates over a wall face, each a little proud of it and riveted
    round its edge, some painted over in another colour, some welded where
    they meet. side is -1 for the front face, +1 for the back."""
    r = rng(style, f"{p.name}{side}")
    cols = 2
    w, h = (x1 - x0) / cols, (z1 - z0) / rows
    y = side * 0.1
    facing = "-Y" if side < 0 else "+Y"
    plates = []
    for c in range(cols):
        for row in range(rows):
            a = (x0 + c * w + 0.015, z0 + row * h + 0.015)
            b = (x0 + (c + 1) * w - 0.015, z0 + (row + 1) * h - 0.015)
            if skip and skip(a, b):
                continue
            mat = r.choice(REPAINTS) if r.random() < style.repaint else "Hull alloy"
            # Some proud of the others, like they were patched over.
            lift = 0.015 + 0.01 * (row % 2)
            face = y + side * lift
            p.span((a[0], min(y, face), a[1]), (b[0], max(y, face), b[1]), mat)
            # Rivets round the edge, sunk into the plate's face.
            for (u0, v0), (u1, v1) in (((a[0] + 0.035, a[1] + 0.035), (b[0] - 0.035, a[1] + 0.035)),
                                       ((a[0] + 0.035, b[1] - 0.035), (b[0] - 0.035, b[1] - 0.035)),
                                       ((a[0] + 0.035, a[1] + 0.035), (a[0] + 0.035, b[1] - 0.035)),
                                       ((b[0] - 0.035, a[1] + 0.035), (b[0] - 0.035, b[1] - 0.035))):
                _bolts_along(p, (u0, face, v0), (u1, face, v1), 0.11, facing, mat, r=0.009, proud=0.004)
            plates.append((a, b, face, mat))
    # Weld beads along some of the seams between plates.
    for (a, b, face, _) in plates:
        if r.random() < 0.4 and b[1] < z1 - 0.1:
            p.cable((a[0], y + side * 0.006, b[1] + 0.015), (b[0], y + side * 0.006, b[1] + 0.015), 0.008, "Rust")
    # A vent in one of the lower plates, now and then.
    if plates and r.random() < 0.45:
        (a, b, face, mat) = plates[0]
        cx = 0.5 * (a[0] + b[0])
        _louvres(p, cx - 0.22, cx + 0.22, a[1] + 0.25, a[1] + 0.55, face, side, "Hull dark", 6)
    if number:
        with p.painted():
            p.stencil(number, (0.5 * (x0 + x1) + 0.45, y + side * 0.031, 1.95), 0.22, "Stencil white",
                      facing=facing)


def _wall_frame(p: Piece, door: float = 0.0) -> None:
    """The ribs at the wall's ends, the kick strip and the top stringer, on
    both faces, with their bolts; the kick strip stops door metres either
    side of the middle, for a doorway."""
    for side in (-1, 1):
        y0, y1 = sorted((side * 0.1, side * 0.15))
        yf = side * 0.15
        facing = "-Y" if side < 0 else "+Y"
        for x in (-H, H - 0.12):
            p.span((x, y0, 0), (x + 0.12, y1, DECK - 0.1), "Hull dark")
            _bolts_along(p, (x + 0.06, yf, 0.3), (x + 0.06, yf, DECK - 0.3), 0.3, facing)
        if door:
            for x0, x1 in ((-H + 0.12, -door), (door, H - 0.12)):
                p.span((x0, y0, 0), (x1, y1, 0.18), "Hull dark")
        else:
            p.span((-H + 0.12, y0, 0), (H - 0.12, y1, 0.18), "Hull dark")
            _bolts_along(p, (-H + 0.25, yf, 0.09), (H - 0.25, yf, 0.09), 0.25, facing)
        p.span((-H + 0.12, y0, DECK - 0.22), (H - 0.12, y1, DECK - 0.1), "Hull dark")
        _bolts_along(p, (-H + 0.25, yf, DECK - 0.16), (H - 0.25, yf, DECK - 0.16), 0.25, facing)


def _wall_core(p: Piece) -> None:
    """The wall's body (the plates sit on its faces) and its capping."""
    p.span((-H, -0.1, 0), (H, 0.1, DECK - 0.1), "Hull alloy")
    p.span((-H, -0.15, DECK - 0.1), (H, 0.15, DECK), "Hull dark")


def _wall_number(style: Style, name: str) -> str:
    r = rng(style, name + "number")
    return f"{r.randint(1, 9)}-{r.randint(10, 99)}"


def hull_wall(style: Style) -> Piece:
    """2 m of hull wall, a deck high and 0.3 m thick: riveted plates on both
    faces (some repainted, some welded), ribs and stringers bolted round
    them, a vent here and there and a frame number stencilled on. The game
    draws a box with all of it baked on."""
    p = Piece("hull_wall", "kit", "2 × 3.6 m wall, 0.3 m thick: kickable")
    p.budget = 500
    p.collider((GRID + SEAM, 0.3, DECK), (0, 0, DECK / 2))
    _wall_core(p)
    _wall_frame(p)
    for side in (-1, 1):
        _plates(p, style, side, -H + 0.12, H - 0.12, 0.18, DECK - 0.22, 3,
                number=_wall_number(style, p.name) if side < 0 else None)
    with p.lowpoly():
        p.span((-H, -0.15, 0), (H, 0.15, DECK), "Hull alloy")
    return p


def hull_wall_port(style: Style) -> Piece:
    """A hull wall with a round viewport: a heavy bolted collar, a brass
    ring, fogged glass held by six dogs."""
    p = Piece("hull_wall_port", "kit", "2 × 3.6 m wall with a viewport")
    p.budget = 500
    p.collider((GRID + SEAM, 0.3, DECK), (0, 0, DECK / 2))
    _wall_core(p)
    _wall_frame(p)
    centre, radius = 1.75, 0.45

    def clear(a, b):
        return abs((a[0] + b[0]) / 2) < 0.6 and a[1] < centre + radius and b[1] > centre - radius

    for side in (-1, 1):
        _plates(p, style, side, -H + 0.12, H - 0.12, 0.18, DECK - 0.22, 3, skip=clear if side < 0 else None)
    # The port: a thick dark collar, a brass ring, the glass and its dogs.
    p.cyl(radius + 0.16, 0.11, (0, -0.145, centre), "Hull dark", rot=(90, 0, 0), segments=16)
    for i in range(16):
        a = math.tau * (i + 0.5) / 16
        _bolt(p, (math.cos(a) * (radius + 0.11), -0.2, centre + math.sin(a) * (radius + 0.11)), "-Y")
    p.cyl(radius + 0.04, 0.05, (0, -0.21, centre), "Brass", rot=(90, 0, 0), segments=16)
    p.cyl(radius - 0.02, 0.02, (0, -0.245, centre), "Glass", rot=(90, 0, 0), segments=16)
    for i in range(6):
        a = math.tau * i / 6
        c = (math.cos(a) * (radius + 0.02), -0.25, centre + math.sin(a) * (radius + 0.02))
        p.box((0.05, 0.02, 0.11), c, "Hull alloy", rot=(0, -math.degrees(a), 0))
        p.cyl(0.018, 0.02, (c[0], -0.262, c[2]), "Steel", rot=(90, 0, 0), segments=4)
    with p.painted():
        p.stencil(_wall_number(style, p.name), (0.45, -0.122, 0.75), 0.18, "Stencil white")
    with p.lowpoly():
        p.span((-H, -0.15, 0), (H, 0.15, DECK), "Hull alloy")
        p.cyl(radius + 0.16, 0.12, (0, -0.21, centre), "Hull dark", rot=(90, 0, 0), segments=5)
    return p


def bulkhead_door(style: Style) -> Piece:
    """A wall with a 1.2 × 2.2 m doorway: the ship's own bulkhead hatch,
    long since jammed open against the wall, its frame striped for hazard."""
    p = Piece("bulkhead_door", "kit", "2 m wall, 1.2 × 2.2 m doorway")
    p.budget = 500
    half, top, f = 0.6, 2.2, 0.14
    # The colliders, as the wall's three solid parts.
    for lo, hi in (((-H, -0.1, 0), (-half, 0.1, DECK)), ((half, -0.1, 0), (H, 0.1, DECK)),
                   ((-half, -0.1, top), (half, 0.1, DECK))):
        p.collider([hi[i] - lo[i] for i in range(3)], [(hi[i] + lo[i]) / 2 for i in range(3)])
    p.span((-H, -0.1, 0), (-half - f, 0.1, DECK - 0.1), "Hull alloy")
    p.span((half + f, -0.1, 0), (H, 0.1, DECK - 0.1), "Hull alloy")
    p.span((-half - f, -0.1, top + f), (half + f, 0.1, DECK - 0.1), "Hull alloy")
    p.span((-H, -0.15, DECK - 0.1), (H, 0.15, DECK), "Hull dark")
    _wall_frame(p, door=half + f)
    for side in (-1, 1):
        _plates(p, style, side, -H + 0.12, H - 0.12, 0.18, DECK - 0.22, 3,
                skip=lambda a, b: a[0] < half + f + 0.05 and b[0] > -half - f - 0.05 and a[1] < top + f + 0.1)
    # The frame, a thick collar proud of both faces.
    p.span((-half - f, -0.2, 0.02), (-half, 0.2, top + f), "Crew grey")
    p.span((half, -0.2, 0.02), (half + f, 0.2, top + f), "Crew grey")
    p.span((-half, -0.2, top), (half, 0.2, top + f), "Crew grey")
    for side, facing in ((-1, "-Y"), (1, "+Y")):
        for x in (-half - f / 2, half + f / 2):
            _bolts_along(p, (x, side * 0.2, 0.15), (x, side * 0.2, top - 0.05), 0.3, facing)
        _bolts_along(p, (-half + 0.1, side * 0.2, top + f / 2), (half - 0.1, side * 0.2, top + f / 2), 0.3, facing)
    # The sill, worn bright.
    p.span((-half - f, -0.2, 0), (half + f, 0.2, 0.02), "Steel")
    # The hatch: a heavy leaf swung back on its hinges, its locking wheel.
    leaf_x = half + f + 0.04
    p.span((leaf_x, -0.36, 0.06), (leaf_x + 0.08, -0.2, top - 0.02), "Hull dark")
    p.span((leaf_x + 0.08, -0.34, 0.12), (leaf_x + 0.1, -0.22, top - 0.08), "Hull alloy")
    for z in (0.4, top - 0.4):
        p.cyl(0.035, 0.18, (half + f + 0.02, -0.25, z), "Steel", segments=4)
    p.torus(0.16, 0.02, (leaf_x + 0.15, -0.28, top / 2), "Crew orange", rot=(0, 90, 0), segments=10, sides=4)
    for i in range(3):
        p.box((0.02, 0.3, 0.02), (leaf_x + 0.15, -0.28, top / 2), "Crew orange", rot=(i * 60, 0, 0))
    p.cyl(0.03, 0.06, (leaf_x + 0.12, -0.28, top / 2), "Hull dark", rot=(0, 90, 0), segments=4)
    with p.painted():
        # Hazard stripes up the frame's front face.
        for x in (-half - f, half):
            for i in range(8):
                p.box((f - 0.03, 0.004, 0.11), (x + f / 2, -0.212, 0.15 + i * 0.27), "Hazard yellow", rot=(0, 35, 0))
        p.stencil(_wall_number(style, p.name), (0, -0.122, top + 0.55), 0.3, "Stencil white")
    with p.lowpoly():
        p.span((-H, -0.15, 0), (-half - f, 0.15, DECK), "Hull alloy")
        p.span((half + f, -0.15, 0), (H, 0.15, DECK), "Hull alloy")
        p.span((-half - f, -0.15, top + f), (half + f, 0.15, DECK), "Hull alloy")
        p.span((-half - f, -0.2, 0), (-half, 0.2, top + f), "Crew grey")
        p.span((half, -0.2, 0), (half + f, 0.2, top + f), "Crew grey")
        p.span((-half, -0.2, top), (half, 0.2, top + f), "Crew grey")
        p.span((leaf_x, -0.36, 0.06), (leaf_x + 0.1, -0.2, top - 0.02), "Hull dark")
    return p


def rib_pillar(style: Style) -> Piece:
    """One of the ship's ribs, standing free where the decks were cut away:
    an I-beam a deck high, riveted down its flanges, stiffened across its
    web, on a bolted foot, with a band of Crew orange at eye height and its
    frame number."""
    p = Piece("rib_pillar", "kit", "0.4 × 0.4 × 3.6 m I-beam")
    p.budget = 500
    s, t = 0.2, 0.05
    p.collider((0.4, 0.4, DECK), (0, 0, DECK / 2))
    p.span((-s, -s, 0.12), (s, -s + t, DECK - 0.12), "Hull dark")
    p.span((-s, s - t, 0.12), (s, s, DECK - 0.12), "Hull dark")
    p.span((-0.03, -s + t, 0.12), (0.03, s - t, DECK - 0.12), "Hull alloy")
    for z in (0.9, 1.8, 2.7):
        for sx in (-1, 1):
            x0, x1 = sorted((sx * 0.03, sx * (s - 0.02)))
            p.span((x0, -s + t, z - 0.01), (x1, s - t, z + 0.01), "Hull alloy")
    for y, facing in ((-s, "-Y"), (s, "+Y")):
        for x in (-0.13, 0.13):
            _bolts_along(p, (x, y, 0.3), (x, y, DECK - 0.3), 0.15, facing, "Hull dark", r=0.01, proud=0.005)
    # A foot and a cap, bolted down and up.
    for z0, z1 in ((0, 0.12), (DECK - 0.12, DECK)):
        p.span((-s - 0.04, -s - 0.04, z0), (s + 0.04, s + 0.04, z1), "Hull alloy")
    for sx in (-1, 1):
        for sy in (-1, 1):
            _bolt(p, (sx * (s + 0.015), sy * (s + 0.015), 0.12), "+Z", r=0.016, proud=0.01)
            _bolt(p, (sx * (s + 0.015), sy * (s + 0.015), DECK - 0.12), "-Z", r=0.016, proud=0.01)
    with p.painted():
        for y0, y1 in ((-s - 0.016, -s - 0.012), (s + 0.012, s + 0.016)):
            p.span((-s, y0, 1.5), (s, y1, 1.7), "Crew orange")
        p.stencil(str(rng(style, p.name).randint(10, 99)), (0, -s - 0.009, 2.1), 0.16, "Stencil white")
    with p.lowpoly():
        p.span((-s, -s, 0.12), (s, -s + t, DECK - 0.12), "Hull dark")
        p.span((-s, s - t, 0.12), (s, s, DECK - 0.12), "Hull dark")
        p.span((-0.03, -s + t, 0.12), (0.03, s - t, DECK - 0.12), "Hull alloy")
        p.span((-s - 0.04, -s - 0.04, 0), (s + 0.04, s + 0.04, 0.12), "Hull alloy")
        p.span((-s - 0.04, -s - 0.04, DECK - 0.12), (s + 0.04, s + 0.04, DECK), "Hull alloy")
    return p


# Catwalks, stairs and ladders -------------------------------------------------

def catwalk(style: Style) -> Piece:
    """2 m of catwalk, its deck a deck up (3.6 m) and exactly 1.2 m wide, as
    its collider is: grating between two channels, bolted, with a knee
    brace under each end for a wall or rib to hold."""
    p = Piece("catwalk", "kit", "2 × 1.2 m catwalk, top at 3.6 m")
    p.budget = 500
    w = 0.6
    p.collider((GRID, 2 * w, 0.15), (0, 0, DECK - 0.075))
    # The channels: C-sections along both edges, lips turned in.
    for y0, y1, lip in ((-w, -w + 0.012, 1), (w - 0.012, w, -1)):
        p.span((-H, y0, DECK - 0.22), (H, y1, DECK), "Crew grey")
        yl = y1 if lip > 0 else y0
        for z in (DECK - 0.22, DECK - 0.012):
            p.span((-H, min(yl, yl + lip * 0.05), z), (H, max(yl, yl + lip * 0.05), z + 0.012), "Crew grey")
    for y, facing in ((-w, "-Y"), (w, "+Y")):
        _bolts_along(p, (-H + 0.15, y, DECK - 0.11), (H - 0.15, y, DECK - 0.11), 0.35, facing)
    # The grating: bearing bars across, cross rods along.
    x = -H + 0.02
    while x < H - 0.01:
        p.span((x - 0.006, -w + 0.062, DECK - 0.04), (x + 0.006, w - 0.062, DECK - 0.004), "Grating")
        x += 0.045
    for y in (-0.4, -0.2, 0.0, 0.2, 0.4):
        p.span((-H, y - 0.005, DECK - 0.016), (H, y + 0.005, DECK - 0.004), "Grating")
    # Cross members under each end, and the knee braces back to the wall.
    for x in (-H + 0.1, H - 0.1):
        p.span((x - 0.04, -w + 0.012, DECK - 0.13), (x + 0.04, w - 0.012, DECK - 0.05), "Hull dark")
        p.box((0.05, 0.05, 0.75), (x, 0.32, DECK - 0.45), "Hull dark", rot=(-40, 0, 0))
        p.span((x - 0.06, w - 0.02, DECK - 0.82), (x + 0.06, w, DECK - 0.62), "Hull dark")
    with p.painted():
        p.span((-H + 0.01, -w - 0.009, DECK - 0.2), (H - 0.01, -w - 0.005, DECK - 0.17), "Hazard yellow")
    with p.lowpoly():
        p.span((-H, -w, DECK - 0.22), (H, w, DECK), "Crew grey")
        for x in (-H + 0.1, H - 0.1):
            p.box((0.05, 0.05, 0.75), (x, 0.32, DECK - 0.45), "Hull dark", rot=(-40, 0, 0))
    return p


def railing(style: Style) -> Piece:
    """2 m of railing, 0.95 m high: low and deep enough (0.2 m) to vault.
    Round posts on bolted base plates, a top rail and a knee rail, and a toe
    board along the bottom."""
    p = Piece("railing", "kit", "2 m railing, 0.95 m high: vaultable")
    p.budget = 500
    top = 0.95
    p.collider((GRID, 0.2, top), (0, 0, top / 2))
    for x in (-H + 0.05, 0, H - 0.05):
        p.cyl(0.025, top - 0.03, (x, 0, 0.012 + (top - 0.042) / 2), "Crew orange", segments=6)
        p.span((x - 0.06, -0.05, 0), (x + 0.06, 0.05, 0.012), "Hull dark")
        for sx in (-1, 1):
            _bolt(p, (x + sx * 0.04, 0.03, 0.012), "+Z", r=0.008, proud=0.005)
        for z in (0.5, top - 0.03):
            p.cyl(0.033, 0.05, (x, 0, z), "Crew grey", segments=6)
    p.cyl(0.024, GRID, (0, 0, top - 0.03), "Crew orange", rot=(0, 90, 0), segments=6)
    p.cyl(0.018, GRID, (0, 0, 0.5), "Crew orange", rot=(0, 90, 0), segments=6)
    p.span((-H, -0.006, 0.012), (H, 0.006, 0.13), "Hull dark")
    with p.lowpoly():
        for x in (-H + 0.05, 0, H - 0.05):
            p.cyl(0.027, top - 0.03, (x, 0, (top - 0.03) / 2), "Crew orange", segments=2)
        p.cyl(0.027, GRID, (0, 0, top - 0.03), "Crew orange", rot=(0, 90, 0), segments=2)
        p.cyl(0.021, GRID, (0, 0, 0.5), "Crew orange", rot=(0, 90, 0), segments=2)
        p.span((-H, -0.008, 0), (H, 0.008, 0.13), "Hull dark")
    return p


# The stairs: twelve 0.3 m steps up a deck, 0.5 m deep, the twelfth level
# with the deck above; walked as a ramp through the treads at 33°, inside
# the controller's 45°.
STAIR_RUN, STAIR_STEPS = 3 * GRID, 12


def stairs(style: Style) -> Piece:
    """A steel stair up a deck: grating treads with orange nosings between
    two bolted stringers, and a handrail each side on posts."""
    p = Piece("stairs", "kit", "1.2 m wide, 3.6 m up over 6 m, rising to +Y")
    p.budget = 1500
    w = 0.6
    tread = STAIR_RUN / STAIR_STEPS
    rise = DECK / STAIR_STEPS
    y0 = -STAIR_RUN / 2
    run = STAIR_RUN - tread
    length = math.hypot(run, DECK)
    a = math.atan2(DECK, run)
    mid_y, mid_z = y0 + run / 2, DECK / 2
    # The colliders: the landing, and the ramp through the treads.
    p.collider((2 * w, tread, 0.15), (0, STAIR_RUN / 2 - tread / 2, DECK - 0.075))
    t = 0.2
    p.collider((2 * w, length, t), (0, mid_y + t / 2 * math.sin(a), mid_z - t / 2 * math.cos(a)), rot=(math.degrees(a), 0, 0))
    for i in range(1, STAIR_STEPS + 1):
        z = rise * i
        ya, yb = y0 + tread * (i - 1), y0 + tread * i
        # A grating tread in a frame, its nosing at the front.
        p.span((-w + 0.08, ya + 0.04, z - 0.05), (w - 0.08, yb, z - 0.035), "Hull dark")
        x = -w + 0.1
        while x < w - 0.09:
            p.span((x - 0.006, ya + 0.04, z - 0.035), (x + 0.006, yb - 0.012, z - 0.002), "Grating")
            x += 0.045
        p.span((-w + 0.08, ya, z - 0.05), (w - 0.08, ya + 0.04, z), "Crew orange")
        p.span((-w + 0.08, yb - 0.012, z - 0.035), (w - 0.08, yb, z - 0.002), "Hull dark")
    # Stringers along the slope, bolted to each tread.
    for x in (-w, w - 0.08):
        p.box((0.08, length + 0.2, 0.3), (x + 0.04, mid_y + 0.1, mid_z - 0.05), "Hull dark", rot=(math.degrees(a), 0, 0))
    for i in range(1, STAIR_STEPS + 1):
        for x, facing in ((-w, "-X"), (w, "+X")):
            _bolt(p, (x, y0 + tread * (i - 0.5), rise * i - 0.12), facing, r=0.012, proud=0.008)
    for x in (-w + 0.04, w - 0.04):
        p.tube([(x, y0, 0.95), (x, y0 + run, DECK + 0.95), (x, STAIR_RUN / 2, DECK + 0.95)], 0.025, "Crew orange", segments=6)
        for i in range(0, STAIR_STEPS, 3):
            y = y0 + tread * (i + 0.5)
            z = rise * (i + 1)
            p.span((x - 0.02, y - 0.02, z), (x + 0.02, y + 0.02, z + 0.95 + (y - y0 - tread * (i + 0.5)) * DECK / run), "Crew orange")
    with p.lowpoly():
        for i in range(1, STAIR_STEPS + 1):
            z = rise * i
            ya, yb = y0 + tread * (i - 1), y0 + tread * i
            p.span((-w + 0.08, ya, z - 0.05), (w - 0.08, yb, z), "Grating")
        for x in (-w, w - 0.08):
            p.box((0.08, length + 0.2, 0.3), (x + 0.04, mid_y + 0.1, mid_z - 0.05), "Hull dark", rot=(math.degrees(a), 0, 0))
        for x in (-w + 0.04, w - 0.04):
            p.box((0.05, length, 0.05), (x, mid_y, mid_z + 0.95), "Crew orange", rot=(math.degrees(a), 0, 0))
            p.span((x - 0.025, y0 + run, DECK + 0.925), (x + 0.025, STAIR_RUN / 2, DECK + 0.975), "Crew orange")
            for i in range(0, STAIR_STEPS, 3):
                y = y0 + tread * (i + 0.5)
                z = rise * (i + 1)
                p.span((x - 0.02, y - 0.02, z), (x + 0.02, y + 0.02, z + 0.95), "Crew orange")
    return p


# The ladder, matched to the climb clip (tools/makehuman/traversal.py):
# rails 0.48 m apart and 7 cm square, rungs every 0.3 m from 0.3 m, the
# climber 0.37 m in front of the rails, and the rails 0.28 m in front of
# what they're fixed to.
LADDER_RAIL, LADDER_AXIS, LADDER_STANDOFF = 0.24, 0.37, 0.28


def ladder(style: Style) -> Piece:
    """A ladder a deck high: square rails, round rungs with grip rings worn
    bright, and standoff brackets bolted to whatever it climbs to. Its back
    (0.28 m behind the rails) goes against the edge it climbs to; it's
    climbed from the front."""
    p = Piece("ladder", "kit", "3.6 m ladder: rails 0.48 m apart, rungs every 0.3 m")
    p.budget = 1500
    top = DECK + 0.04
    for x in (-LADDER_RAIL, LADDER_RAIL):
        p.collider((0.07, 0.07, top), (x, 0, top / 2))
        p.span((x - 0.035, -0.035, 0.01), (x + 0.035, 0.035, top), "Crew orange")
        _bolts_along(p, (x, -0.035, 0.15), (x, -0.035, top - 0.15), 0.3, "-Y", r=0.008, proud=0.004)
        # A foot plate under each rail.
        p.span((x - 0.06, -0.06, 0), (x + 0.06, 0.06, 0.01), "Hull dark")
    z = 0.3
    while z < DECK - 0.05:
        p.cyl(0.02, 2 * LADDER_RAIL - 0.07, (0, 0, z), "Hull alloy", rot=(0, 90, 0), segments=6)
        for x in (-0.12, -0.04, 0.04, 0.12):
            p.cyl(0.026, 0.012, (x, 0, z), "Steel", rot=(0, 90, 0), segments=6)
        z += 0.3
    # Standoffs to the wall behind, with their wall plates.
    for z in (0.8, 2.2, DECK - 0.15):
        for x in (-LADDER_RAIL, LADDER_RAIL):
            p.span((x - 0.025, 0.035, z - 0.025), (x + 0.025, LADDER_STANDOFF - 0.01, z + 0.025), "Hull dark")
            p.span((x - 0.05, LADDER_STANDOFF - 0.01, z - 0.06), (x + 0.05, LADDER_STANDOFF, z + 0.06), "Hull dark")
    p.ladder(
        bottom=(0, -LADDER_AXIS, 0), top=(0, -LADDER_AXIS, top), facing=(0, 1, 0),
        bottom_exit=(0, -LADDER_AXIS - 0.65, 0), top_exit=(0, 0.63, DECK))
    with p.lowpoly():
        for x in (-LADDER_RAIL, LADDER_RAIL):
            p.span((x - 0.035, -0.035, 0), (x + 0.035, 0.035, top), "Crew orange")
        z = 0.3
        while z < DECK - 0.05:
            p.cyl(0.023, 2 * LADDER_RAIL - 0.07, (0, 0, z), "Hull alloy", rot=(0, 90, 0), segments=2)
            z += 0.3
        for z in (0.8, 2.2, DECK - 0.15):
            for x in (-LADDER_RAIL, LADDER_RAIL):
                p.span((x - 0.025, 0.035, z - 0.025), (x + 0.025, LADDER_STANDOFF, z + 0.025), "Hull dark")
    return p


# Services -------------------------------------------------------------------

def pipe_run(style: Style) -> Piece:
    """2 m of pipes along a wall, above head height: three lines on bolted
    brackets with flanged joints, one with a hand wheel and a gauge, banded
    in their service colours. Its back goes on a wall's face."""
    p = Piece("pipe_run", "kit", "2 m of wall pipes at 2.5–3.2 m; no collider")
    p.budget = 800
    r = rng(style, p.name)
    lines = [(2.55, 0.09, "Rust"), (2.85, 0.06, "Crew orange"), (3.15, 0.11, r.choice(["Hull dark", "Repaint teal"]))]
    for z, radius, mat in lines:
        p.cyl(radius, GRID, (0, -0.2, z), mat, rot=(0, 90, 0), segments=6)
        for x in (-H + 0.04, H - 0.04):
            p.cyl(radius + 0.03, 0.06, (x, -0.2, z), "Hull dark", rot=(0, 90, 0), segments=6)
    # Brackets: a channel up the wall, a saddle under each pipe.
    for x in (-0.7, 0.7):
        p.span((x - 0.04, -0.06, 2.35), (x + 0.04, 0, 3.35), "Hull dark")
        for z, radius, _ in lines:
            p.span((x - 0.03, -0.2, z - radius - 0.05), (x + 0.03, -0.06, z - radius - 0.028), "Hull dark")
            p.torus(radius + 0.012, 0.012, (x, -0.2, z), "Steel", rot=(0, 90, 0), segments=4, sides=2)
        _bolt(p, (x, -0.06, 2.42), "-Y")
        _bolt(p, (x, -0.06, 3.28), "-Y")
    # The hand wheel and a gauge on the middle line.
    p.cyl(0.04, 0.12, (0.2, -0.29, 2.85), "Hull dark", rot=(90, 0, 0), segments=4)
    p.torus(0.13, 0.014, (0.2, -0.36, 2.85), "Fabric red", rot=(90, 0, 0), segments=8, sides=3)
    for i in range(3):
        p.box((0.24, 0.016, 0.016), (0.2, -0.36, 2.85), "Fabric red", rot=(0, i * 60, 0))
    p.cyl(0.015, 0.12, (-0.35, -0.2, 2.95), "Steel", segments=4)
    p.cyl(0.05, 0.03, (-0.35, -0.215, 3.04), "Steel", rot=(90, 0, 0), segments=6)
    p.cyl(0.042, 0.012, (-0.35, -0.236, 3.04), "Paper", rot=(90, 0, 0), segments=6)
    with p.lowpoly():
        for z, radius, _ in lines:
            p.cyl(radius + 0.004, GRID, (0, -0.2, z), "Hull dark", rot=(0, 90, 0), segments=3)
            for x in (-H + 0.04, H - 0.04):
                p.cyl(radius + 0.03, 0.06, (x, -0.2, z), "Hull dark", rot=(0, 90, 0), segments=3)
        for x in (-0.7, 0.7):
            p.span((x - 0.04, -0.06, 2.35), (x + 0.04, 0, 3.35), "Hull dark")
            for z, radius, _ in lines:
                p.span((x - 0.03, -0.2, z - radius - 0.05), (x + 0.03, -0.06, z - radius - 0.028), "Hull dark")
        p.cyl(0.14, 0.04, (0.2, -0.36, 2.85), "Fabric red", rot=(90, 0, 0), segments=3)
        p.cyl(0.05, 0.05, (-0.35, -0.225, 3.04), "Steel", rot=(90, 0, 0), segments=3)
    with p.painted():
        # Service bands and flow arrows.
        for z, radius, _ in lines:
            p.cyl(radius + 0.006, 0.08, (-0.15, -0.2, z), "Stencil white", rot=(0, 90, 0), segments=6)
            p.prism([(0, 0), (0.12, 0.03), (0, 0.06)], 0.004, (0.5, -0.2 - radius - 0.007, z - 0.03), "Stencil white")
    return p


def low_duct(style: Style) -> Piece:
    """An air duct laid across a 2 m gap, its underside 1.075 m up: too low
    to walk under, so it's slid under. Panels riveted along their seams, an
    access hatch with its latches, hazard chevrons both sides."""
    p = Piece("low_duct", "kit", "2 m duct, underside at 1.075 m: slide under")
    p.budget = 1500
    bottom, height, w = 1.075, 0.5, 0.35
    p.collider((GRID, 2 * w, height), (0, 0, bottom + height / 2))
    p.span((-H + 0.06, -w, bottom), (H - 0.06, w, bottom + height), "Hull alloy")
    # Seams round it every half metre, riveted both sides of each.
    for x in (-0.5, 0.0, 0.5):
        p.span((x - 0.02, -w - 0.015, bottom - 0.015), (x + 0.02, w + 0.015, bottom + height + 0.015), "Hull dark")
        for side, facing in ((-1, "-Y"), (1, "+Y")):
            for dx in (-0.06, 0.06):
                _bolts_along(p, (x + dx, side * w, bottom + 0.06), (x + dx, side * w, bottom + height - 0.06), 0.1,
                             facing, "Hull alloy", r=0.007, proud=0.004)
    # The access hatch on the front, latched.
    p.span((-0.4, -w - 0.012, bottom + 0.08), (-0.12, -w, bottom + 0.42), "Hull alloy")
    for z in (bottom + 0.14, bottom + 0.36):
        p.span((-0.11, -w - 0.03, z - 0.02), (-0.075, -w, z + 0.02), "Steel")
    # Its flanged ends, which sit on whatever it spans.
    for x in (-H, H - 0.06):
        p.span((x, -w - 0.04, bottom - 0.04), (x + 0.06, w + 0.04, bottom + height + 0.04), "Hull dark")
        for side, facing in ((-1, "-Y"), (1, "+Y")):
            _bolts_along(p, (x + 0.03, side * (w + 0.04), bottom), (x + 0.03, side * (w + 0.04), bottom + height),
                         0.12, facing)
    with p.painted():
        for side in (-1, 1):
            y = side * (w + 0.008)
            for i in range(6):
                x = 0.08 + i * 0.13
                p.box((0.06, 0.004, height * 0.45), (x, y, bottom + height / 2), "Hazard yellow", rot=(0, 35, 0))
        p.stencil("11", (-0.78, -w - 0.008, bottom + 0.25), 0.12, "Stencil white")
    with p.lowpoly():
        p.span((-H + 0.06, -w - 0.015, bottom - 0.015), (H - 0.06, w + 0.015, bottom + height + 0.015), "Hull alloy")
        for x in (-H, H - 0.06):
            p.span((x, -w - 0.04, bottom - 0.04), (x + 0.06, w + 0.04, bottom + height + 0.04), "Hull dark")
    return p


PIECES = [deck_floor, deck_slab, hull_wall, hull_wall_port, bulkhead_door, rib_pillar, catwalk, railing, stairs,
          ladder, pipe_run, low_duct]
