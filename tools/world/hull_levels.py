"""The Hull's levels: what makes its decks read as stacked, one over
another. The ceiling of a deck seen from below, a hatch with a ladder down
to the deck under it, a stairwell going down, a freight lift through the
decks, and a grated vent over a shaft lit from the air plant below (see
docs/settlement.md: the Stacks above, the markets in the middle, the Crew's
lower decks under them).

Like the rest of the Hull kit they're on the 2 m grid with a 3.6 m deck
height, each standing on its origin with a floor's top at z = 0 (as
deck_slab), its front along -Y. What goes down to the deck below goes to
z = -3.6. The ones tiled by the dozen have low-poly stand-ins, their detail
baked onto them (kit.Piece.lowpoly)."""
import math

from hull_kit import H, LADDER_AXIS, LADDER_RAIL, LADDER_STANDOFF, _bolt, _bolts_along
from kit import DECK, GRID, REPAINTS, SLAB, Piece, Style, rng

CATEGORY = "Hull decks"


# Shared parts -----------------------------------------------------------------

def _tread(p: Piece, r, x0, x1, y0, y1, mat=None, paint: bool = False) -> None:
    """Tread plate from (x0, y0) to (x1, y1), its top at the floor: a plate
    with a diamond tread pressed into it, riveted round its edge. The tread
    is hundreds of little boxes: on a piece without a stand-in (paint=True)
    it's paint, baked in, not geometry the finish would bevel box by box."""
    mat = mat or (r.choice(REPAINTS) if r.random() < 0.25 else "Hull alloy")
    p.span((x0, y0, -0.02), (x1, y1, 0), mat)
    i = 0
    y = y0 + 0.05
    while y < y1 - 0.04:
        x = x0 + 0.05 + (0.04 if i % 2 else 0)
        while x < x1 - 0.04:
            if paint:
                with p.painted():
                    p.box((0.045, 0.012, 0.004), (x, y, 0.006), "Steel", rot=(0, 0, 45 if (i + round(x * 25)) % 2 else -45))
            else:
                p.box((0.045, 0.012, 0.006), (x, y, 0.0), mat, rot=(0, 0, 45 if (i + round(x * 25)) % 2 else -45))
            x += 0.08
        y += 0.045
        i += 1
    for a, b in (((x0 + 0.025, y0 + 0.025), (x1 - 0.025, y0 + 0.025)), ((x0 + 0.025, y1 - 0.025), (x1 - 0.025, y1 - 0.025)),
                 ((x0 + 0.025, y0 + 0.025), (x0 + 0.025, y1 - 0.025)), ((x1 - 0.025, y0 + 0.025), (x1 - 0.025, y1 - 0.025))):
        if paint:
            with p.painted():
                _bolts_along(p, (a[0], a[1], 0.002), (b[0], b[1], 0.002), 0.15, "+Z", "Steel", r=0.009, proud=0.004)
        else:
            _bolts_along(p, (a[0], a[1], 0), (b[0], b[1], 0), 0.15, "+Z", mat, r=0.009, proud=0.004)


def _edge_channel(p: Piece, x0, x1, y0, y1) -> None:
    """A bolted channel round a floor's edge (or an opening's), from the top
    of the floor down its thickness, its faces looking out."""
    t = 0.04
    p.span((x0, y0, -SLAB), (x1, y0 + t, 0), "Hull alloy")
    p.span((x0, y1 - t, -SLAB), (x1, y1, 0), "Hull alloy")
    p.span((x0, y0 + t, -SLAB), (x0 + t, y1 - t, 0), "Hull alloy")
    p.span((x1 - t, y0 + t, -SLAB), (x1, y1 - t, 0), "Hull alloy")


def _hazard_edge(p: Piece, a, b, facing: str, z0=-0.1, z1=-0.01, n=0) -> None:
    """Painted hazard stripes along an opening's edge face (the face looking
    facing), from a to b (x or y), 5 mm proud."""
    with p.painted():
        if facing in ("-Y", "+Y"):
            y = a[1]
            length = b[0] - a[0]
            n = n or max(2, round(length / 0.12))
            for k in range(n):
                if k % 2:
                    continue
                xa = a[0] + length * k / n
                xb = a[0] + length * (k + 1) / n
                yy = (y - 0.005, y) if facing == "-Y" else (y, y + 0.005)
                p.span((xa, yy[0], z0), (xb, yy[1], z1), "Hazard yellow")
        else:
            x = a[0]
            length = b[1] - a[1]
            n = n or max(2, round(length / 0.12))
            for k in range(n):
                if k % 2:
                    continue
                ya = a[1] + length * k / n
                yb = a[1] + length * (k + 1) / n
                xx = (x - 0.005, x) if facing == "-X" else (x, x + 0.005)
                p.span((xx[0], ya, z0), (xx[1], yb, z1), "Hazard yellow")


def _railing(p: Piece, a, b, low: bool = True) -> None:
    """A railing from a to b on the floor (z = 0): round posts on bolted base
    plates every metre or so, a top and a knee rail, a toe board, and its
    collider (0.95 m high and 0.2 deep, so it's vaulted, like hull_kit's).
    low builds its stand-in too."""
    top = 0.95
    ax, ay = a
    bx, by = b
    length = math.hypot(bx - ax, by - ay)
    along_x = abs(bx - ax) > abs(by - ay)
    n = max(1, round(length / 1.0))
    posts = [(ax + (bx - ax) * k / n, ay + (by - ay) * k / n) for k in range(n + 1)]
    rot = (0, 90, 0) if along_x else (90, 0, 0)
    mid = ((ax + bx) / 2, (ay + by) / 2)
    for x, y in posts:
        p.cyl(0.025, top - 0.03, (x, y, 0.012 + (top - 0.042) / 2), "Crew orange", segments=6)
        p.span((x - 0.06, y - 0.05, 0), (x + 0.06, y + 0.05, 0.012), "Hull dark")
        for z in (0.5, top - 0.03):
            p.cyl(0.033, 0.064, (x, y, z), "Crew grey", segments=6)
    p.cyl(0.024, length, (mid[0], mid[1], top - 0.03), "Crew orange", rot=rot, segments=6)
    p.cyl(0.018, length, (mid[0], mid[1], 0.5), "Crew orange", rot=rot, segments=6)
    if along_x:
        p.span((min(ax, bx), ay - 0.006, 0.012), (max(ax, bx), ay + 0.006, 0.13), "Hull dark")
        p.collider((length, 0.2, top), (mid[0], mid[1], top / 2))
    else:
        p.span((ax - 0.006, min(ay, by), 0.012), (ax + 0.006, max(ay, by), 0.13), "Hull dark")
        p.collider((0.2, length, top), (mid[0], mid[1], top / 2))
    if low:
        with p.lowpoly():
            for x, y in posts:
                p.cyl(0.027, top - 0.03, (x, y, (top - 0.03) / 2), "Crew orange", segments=2)
            p.cyl(0.027, length, (mid[0], mid[1], top - 0.03), "Crew orange", rot=rot, segments=2)
            p.cyl(0.021, length, (mid[0], mid[1], 0.5), "Crew orange", rot=rot, segments=2)
            if along_x:
                p.span((min(ax, bx), ay - 0.008, 0), (max(ax, bx), ay + 0.008, 0.13), "Hull dark")
            else:
                p.span((ax - 0.008, min(ay, by), 0), (ax + 0.008, max(ay, by), 0.13), "Hull dark")


def _floor_box(p: Piece, x0, x1, y0, y1) -> None:
    """A walkable stretch of floor, top at z = 0: its collider and stand-in."""
    p.collider((x1 - x0, y1 - y0, SLAB), ((x0 + x1) / 2, (y0 + y1) / 2, -SLAB / 2))
    with p.lowpoly():
        p.span((x0, y0, -SLAB), (x1, y1, 0), "Hull alloy")


# The ceiling ------------------------------------------------------------------

def deck_ceiling(style: Style) -> Piece:
    """2×2 m of a deck's ceiling, as it's seen from the deck below: the
    plating's underside between two I-beams, a cable tray and a pipe run
    hung off them, and a caged sodium lamp. Its top (plating, riveted) is at
    z = 0, so it's placed with its top at the floor above; it can be walked
    on like deck_slab. The game draws a few boxes with all of this baked on."""
    p = Piece("deck_ceiling", "kit", "2 × 2 m ceiling, top at 0, hanging to -0.62", budget=200)
    r = rng(style, p.name)
    p.collider((GRID, GRID, SLAB), (0, 0, -SLAB / 2))
    # The plating, on top and underneath, and its edge channel.
    _tread(p, r, -H + 0.04, H - 0.04, -H + 0.04, H - 0.04)
    p.span((-H + 0.04, -H + 0.04, -0.25), (H - 0.04, H - 0.04, -0.02), "Hull dark")
    _edge_channel(p, -H, H, -H, H)
    for y, facing in ((-H, "-Y"), (H, "+Y")):
        _bolts_along(p, (-H + 0.1, y, -0.15), (H - 0.1, y, -0.15), 0.2, facing)
    # The underside: a skin of plates with stiffeners, a seam welded down it.
    p.span((-H + 0.04, -H + 0.04, -SLAB), (H - 0.04, H - 0.04, -0.25), "Crew grey")
    for x in (-0.5, 0.5):
        # Between the beams, not through them.
        for y0, y1 in ((-H + 0.04, -0.63), (-0.47, 0.47), (0.63, H - 0.04)):
            p.span((x - 0.01, y0, -0.33), (x + 0.01, y1, -SLAB), "Crew grey")
    for y0, y1 in ((-H + 0.04, -0.64), (-0.46, 0.46), (0.64, H - 0.04)):  # between the beams
        p.cable((-0.002, y0, -0.302), (-0.002, y1, -0.302), 0.006, "Steel", steps=2)
    # Two I-beams across it, bolted at their flanges.
    for y in (-0.55, 0.55):
        p.span((-H, y - 0.08, -0.32), (H, y + 0.08, -SLAB), "Hull dark")
        p.span((-H, y - 0.012, -0.5), (H, y + 0.012, -0.32), "Hull dark")
        p.span((-H, y - 0.08, -0.52), (H, y + 0.08, -0.5), "Hull dark")
        for x in (-0.9, -0.3, 0.3, 0.9):
            p.span((x - 0.006, y - 0.075, -0.5), (x + 0.006, y + 0.075, -0.32), "Hull dark")
        _bolts_along(p, (-H + 0.1, y - 0.06, -0.52), (H - 0.1, y - 0.06, -0.52), 0.25, "-Z", r=0.008, proud=0.005)
        _bolts_along(p, (-H + 0.1, y + 0.06, -0.52), (H - 0.1, y + 0.06, -0.52), 0.25, "-Z", r=0.008, proud=0.005)
    # A cable tray hung between the beams, cables in it.
    for x in (-0.7, 0.7):
        for y in (-0.18, 0.17):  # down the tray's lips, clear of its cables
            p.span((x - 0.01, y, -0.43), (x + 0.01, y + 0.01, -SLAB), "Steel")
    p.span((-H, -0.18, -0.5), (H, 0.18, -0.49), "Steel")
    for y in (-0.18, 0.17):
        p.span((-H, y, -0.49), (H, y + 0.01, -0.43), "Steel")
    for k, (y, rad, mat) in enumerate(((-0.11, 0.02, "Rubber"), (-0.05, 0.016, "Crew orange"), (0.02, 0.022, "Rubber"),
                                       (0.1, 0.014, "Water blue"))):
        p.cable((-H, y, -0.49 + rad), (H, y + 0.01 * (k % 2), -0.49 + rad), rad, mat, sag=0.0, steps=2, segments=4)
    # A pipe run along the beam, on clamps, with a flange and a valve.
    pipe_y, pipe_z = 0.8, -0.42
    p.tube([(-H, pipe_y, pipe_z), (H, pipe_y, pipe_z)], 0.05, r.choice(["Rust", "Repaint green", "Crew orange"]), segments=8)
    for x in (-0.6, 0.6):
        p.span((x - 0.02, pipe_y - 0.06, pipe_z - 0.06), (x + 0.02, pipe_y + 0.06, -SLAB), "Hull dark")
    p.cyl(0.075, 0.04, (0.15, pipe_y, pipe_z), "Hull dark", rot=(0, 90, 0), segments=10)
    p.cyl(0.012, 0.12, (-0.35, pipe_y, pipe_z - 0.1), "Steel", segments=4)
    p.torus(0.05, 0.01, (-0.35, pipe_y, pipe_z - 0.17), "Fabric red", segments=10, sides=3)
    # The lamp: a sodium tube behind a wire cage, hung off a bracket.
    lx, ly = -0.15, 0.0
    p.span((lx - 0.2, ly - 0.06, -0.39), (lx + 0.2, ly + 0.06, -0.33), "Crew orange")
    p.span((lx - 0.19, ly - 0.05, -0.42), (lx + 0.19, ly + 0.05, -0.39), "Sodium lamp")
    # The cage's bars run up into the bracket, not to the lamp's face.
    for x in (lx - 0.17, lx - 0.085, lx, lx + 0.085, lx + 0.17):
        p.span((x - 0.004, ly - 0.065, -0.44), (x + 0.004, ly + 0.065, -0.36), "Hull dark")
    p.span((lx - 0.2, ly - 0.065, -0.445), (lx + 0.2, ly - 0.057, -0.36), "Hull dark")
    p.span((lx - 0.2, ly + 0.057, -0.445), (lx + 0.2, ly + 0.065, -0.36), "Hull dark")
    with p.painted():
        p.stencil(str(r.randint(10, 99)), (0.62, -H - 0.005, -0.15), 0.1, "Stencil white")
    with p.lowpoly():
        p.span((-H, -H, -SLAB), (H, H, 0), "Hull dark")
        for y in (-0.55, 0.55):
            p.span((-H, y - 0.08, -0.52), (H, y + 0.08, -SLAB), "Hull dark")
        p.span((-H, -0.18, -0.5), (H, 0.18, -0.43), "Steel")
        p.span((-H, pipe_y - 0.05, pipe_z - 0.05), (H, pipe_y + 0.05, pipe_z + 0.05), "Rust")
        p.span((lx - 0.2, ly - 0.065, -0.445), (lx + 0.2, ly + 0.065, -0.33), "Crew orange")
    return p


# Between decks ----------------------------------------------------------------

def deck_hatch(style: Style) -> Piece:
    """A hatch in the deck with a ladder down to the deck below: a 1 × 1.2 m
    opening in a 2×2 m floor, a coaming round it striped in hazard paint,
    the hatch's lid hinged up and over against its stay at the front, and a
    ladder fixed to a back frame on the far side of the opening, from the
    deck below (z = -3.6) up to the floor. It's climbed facing +Y and left
    onto the floor behind it, as hull_kit's ladder is."""
    p = Piece("deck_hatch", "kit", "2 × 2 m floor, 1 × 1.2 m hatch, ladder down 3.6 m", budget=1500)
    r = rng(style, p.name)
    hx, hy0, hy1 = 0.5, -0.6, 0.6  # the opening: x within ±hx, y from hy0 to hy1
    rail_y = hy1 - LADDER_STANDOFF  # the rails, their standoffs back to the opening's far edge
    axis_y = rail_y - LADDER_AXIS
    # The floor round the opening, four stretches, each walkable.
    stretches = [(-H, H, hy1, H), (-H, H, -H, hy0), (-H, -hx, hy0, hy1), (hx, H, hy0, hy1)]
    for x0, x1, y0, y1 in stretches:
        _tread(p, r, x0 + 0.02, x1 - 0.02, y0 + 0.02, y1 - 0.02, "Hull alloy")
        p.span((x0 + 0.02, y0 + 0.02, -0.25), (x1 - 0.02, y1 - 0.02, -0.02), "Hull dark")
        _floor_box(p, x0, x1, y0, y1)
    _edge_channel(p, -H, H, -H, H)
    # The coaming: a lip round the opening, its inside faces striped.
    c = 0.05
    p.span((-hx - c, hy0 - c, -SLAB), (hx + c, hy0, 0.05), "Crew grey")
    p.span((-hx - c, hy1, -SLAB), (hx + c, hy1 + c, 0.05), "Crew grey")
    p.span((-hx - c, hy0, -SLAB), (-hx, hy1, 0.05), "Crew grey")
    p.span((hx, hy0, -SLAB), (hx + c, hy1, 0.05), "Crew grey")
    _hazard_edge(p, (-hx, hy0), (hx, hy0), "+Y", -0.25, 0.04)
    _hazard_edge(p, (-hx, hy1), (hx, hy1), "-Y", -0.25, 0.04)
    _hazard_edge(p, (-hx, hy0), (-hx, hy1), "+X", -0.25, 0.04)
    _hazard_edge(p, (hx, hy0), (hx, hy1), "-X", -0.25, 0.04)
    for x in (-hx - c / 2, hx + c / 2):
        _bolts_along(p, (x, hy0 + 0.05, 0.05), (x, hy1 - 0.05, 0.05), 0.2, "+Z", "Steel", r=0.008, proud=0.006)
    # The lid, hinged at the front edge and standing up past upright against
    # its stay: plate, stiffeners, a wheel to dog it shut.
    lid_t, lid_a = 0.05, 105
    a = math.radians(lid_a)
    hinge = (0, hy0 - c, 0.05)
    lid_len = hy1 - hy0 + 2 * c
    centre = (0, hinge[1] + math.cos(a) * lid_len / 2, hinge[2] + math.sin(a) * lid_len / 2)
    p.box((2 * hx + 2 * c, lid_len, lid_t), centre, "Crew grey", rot=(lid_a, 0, 0))
    n = (0, -math.sin(a), math.cos(a))  # the lid's underside looks this way (rotated -Z)
    under = (centre[0], centre[1] - n[1] * lid_t / 2, centre[2] - n[2] * lid_t / 2)
    for x in (-0.3, 0.3):
        p.box((0.03, lid_len - 0.1, 0.04), (x, under[1] - n[1] * 0.02, under[2] - n[2] * 0.02), "Hull dark", rot=(lid_a, 0, 0))
    p.torus(0.12, 0.012, (under[0], under[1] - n[1] * 0.05, under[2] - n[2] * 0.05), "Fabric red", rot=(lid_a, 0, 0),
            segments=12, sides=3)
    for k in range(3):
        p.box((0.24, 0.012, 0.012), (under[0], under[1] - n[1] * 0.05, under[2] - n[2] * 0.05), "Fabric red",
              rot=(lid_a, 0, 60 * k))
    for x in (-hx + 0.08, hx - 0.08):
        p.cyl(0.025, 0.12, (x, hinge[1], hinge[2] + 0.005), "Steel", rot=(0, 90, 0), segments=6)
    # The stay: a strut from the floor to the lid's back.
    p.tube([(hx + 0.3, hy0 - 0.35, 0.0), (hx - 0.05, centre[1] - n[1] * 0.06, centre[2] - n[2] * 0.06 - 0.25)], 0.012,
           "Hull dark", segments=4)
    p.collider((2 * hx + 2 * c, 0.12, 1.25), (0, hinge[1] - 0.15, 0.62))
    # The ladder: square rails, rungs every 0.3 m from 0.3 m above the deck
    # below, standoffs back to a frame of two flat bars on the far side.
    bottom, top = -DECK, 0.04
    for x in (-LADDER_RAIL, LADDER_RAIL):
        p.collider((0.07, 0.07, top - bottom), (x, rail_y, (top + bottom) / 2))
        p.span((x - 0.035, rail_y - 0.035, bottom + 0.01), (x + 0.035, rail_y + 0.035, top), "Crew orange")
        _bolts_along(p, (x, rail_y - 0.035, bottom + 0.15), (x, rail_y - 0.035, top - 0.15), 0.3, "-Y", r=0.008, proud=0.004)
        p.span((x - 0.06, rail_y - 0.06, bottom), (x + 0.06, rail_y + 0.06, bottom + 0.01), "Hull dark")
        # The back frame the standoffs are bolted to.
        p.span((x - 0.04, hy1 - 0.012, bottom), (x + 0.04, hy1, -SLAB), "Hull dark")
    z = bottom + 0.3
    while z < -0.05:
        p.cyl(0.02, 2 * LADDER_RAIL - 0.07, (0, rail_y, z), "Hull alloy", rot=(0, 90, 0), segments=6)
        for x in (-0.12, -0.04, 0.04, 0.12):
            p.cyl(0.026, 0.012, (x, rail_y, z), "Steel", rot=(0, 90, 0), segments=6)
        z += 0.3
    for z in (bottom + 0.8, bottom + 2.2, -0.45):
        for x in (-LADDER_RAIL, LADDER_RAIL):
            p.span((x - 0.025, rail_y + 0.035, z - 0.025), (x + 0.025, hy1 - 0.012, z + 0.025), "Hull dark")
    p.ladder(bottom=(0, axis_y, bottom), top=(0, axis_y, top), facing=(0, 1, 0),
             bottom_exit=(0, axis_y - 0.65, bottom), top_exit=(0, rail_y + 0.63, 0))
    with p.lowpoly():
        c2 = c
        p.span((-hx - c2, hy0 - c2, -SLAB), (hx + c2, hy0, 0.05), "Crew grey")
        p.span((-hx - c2, hy1, -SLAB), (hx + c2, hy1 + c2, 0.05), "Crew grey")
        p.span((-hx - c2, hy0, -SLAB), (-hx, hy1, 0.05), "Crew grey")
        p.span((hx, hy0, -SLAB), (hx + c2, hy1, 0.05), "Crew grey")
        p.box((2 * hx + 2 * c, lid_len, lid_t), centre, "Crew grey", rot=(lid_a, 0, 0))
        for x in (-LADDER_RAIL, LADDER_RAIL):
            p.span((x - 0.035, rail_y - 0.035, bottom), (x + 0.035, rail_y + 0.035, top), "Crew orange")
            p.span((x - 0.04, hy1 - 0.012, bottom), (x + 0.04, hy1, -SLAB), "Hull dark")
        z = bottom + 0.3
        while z < -0.05:
            p.cyl(0.023, 2 * LADDER_RAIL - 0.07, (0, rail_y, z), "Hull alloy", rot=(0, 90, 0), segments=2)
            z += 0.3
        for z in (bottom + 0.8, bottom + 2.2, -0.45):
            for x in (-LADDER_RAIL, LADDER_RAIL):
                p.span((x - 0.025, rail_y + 0.035, z - 0.025), (x + 0.025, hy1 - 0.012, z + 0.025), "Hull dark")
    return p


# A stairwell: a flight down a deck through a 1.4 m opening, twelve 0.3 m
# risers over 5.4 m (33.7°, inside the controller's 45°); walked as a ramp
# through the nosings, as hull_kit's stairs are.
WELL_RUN, WELL_STEPS = 5.4, 12


def stairwell(style: Style) -> Piece:
    """A stairwell down to the deck below: a 1.4 × 5.6 m opening in a 2 × 6
    m floor, entered from its open -Y end, railed along both sides and
    across the far end, with a steel flight going down to the deck below
    (z = -3.6) at its +Y end: grating treads with orange nosings between
    bolted stringers, a handrail down each side, and a sign at the end."""
    p = Piece("stairwell", "kit", "2 × 6 m floor opening, a flight down 3.6 m toward +Y", budget=3000)
    r = rng(style, p.name)
    L = 3.0  # half the length
    ox, oy1 = 0.7, 2.6  # the opening: x within ±ox, y from -L to oy1
    tread = WELL_RUN / WELL_STEPS
    rise = DECK / WELL_STEPS
    # The floor: a rim either side and a strip across the far end.
    for x0, x1, y0, y1 in ((-H, -ox, -L, L), (ox, H, -L, L), (-ox, ox, oy1, L)):
        _tread(p, r, x0 + 0.02, x1 - 0.02, y0 + 0.02, y1 - 0.02, "Hull alloy")
        p.span((x0 + 0.02, y0 + 0.02, -0.25), (x1 - 0.02, y1 - 0.02, -0.02), "Hull dark")
        _floor_box(p, x0, x1, y0, y1)
    for x0, x1 in ((-H, -ox), (ox, H)):
        _edge_channel(p, x0, x1, -L, L)
    _edge_channel(p, -ox, ox, oy1, L)
    _hazard_edge(p, (-ox, -L), (-ox, oy1), "+X")
    _hazard_edge(p, (ox, -L), (ox, oy1), "-X")
    _hazard_edge(p, (-ox, oy1), (ox, oy1), "-Y")
    # The flight: treads k = 0..10 going down toward +Y; the twelfth riser
    # steps onto the deck below.
    w = 0.6
    y_top = -L
    for k in range(WELL_STEPS - 1):
        z = -rise * (k + 1)
        ya, yb = y_top + tread * k, y_top + tread * (k + 1)
        p.span((-w + 0.08, ya + 0.04, z - 0.05), (w - 0.08, yb, z - 0.035), "Hull dark")
        x = -w + 0.1
        while x < w - 0.09:
            p.span((x - 0.006, ya + 0.04, z - 0.035), (x + 0.006, yb - 0.012, z - 0.002), "Grating")
            x += 0.045
        p.span((-w + 0.08, ya, z - 0.05), (w - 0.08, ya + 0.04, z), "Crew orange")
    run = WELL_RUN
    length = math.hypot(run, DECK)
    a = math.atan2(DECK, run)
    mid_y, mid_z = y_top + run / 2, -DECK / 2
    for x in (-w, w - 0.08):
        p.box((0.08, length + 0.15, 0.3), (x + 0.04, mid_y, mid_z - 0.08), "Hull dark", rot=(-math.degrees(a), 0, 0))
    for k in range(WELL_STEPS - 1):
        for x, facing in ((-w, "-X"), (w, "+X")):
            _bolt(p, (x, y_top + tread * (k + 0.5), -rise * (k + 1) - 0.12), facing, r=0.012, proud=0.008)
    # The ramp collider through the nosings, from the floor's edge down to
    # the deck below.
    t = 0.2
    nz, ny = math.cos(a), math.sin(a)  # the ramp's top looks up and toward +Y
    p.collider((2 * w, length, t), (0, mid_y - ny * t / 2, mid_z - nz * t / 2), rot=(-math.degrees(a), 0, 0))
    # Handrails down each side of the flight, on posts from the stringers.
    for x in (-w + 0.04, w - 0.04):
        p.tube([(x, y_top, 0.95), (x, y_top + run, 0.95 - DECK), (x, y_top + run + 0.3, 0.95 - DECK)], 0.025,
               "Crew orange", segments=6)
        for k in range(0, WELL_STEPS - 1, 3):
            y = y_top + tread * (k + 0.5)
            z = -rise * (k + 1)
            p.span((x - 0.02, y - 0.02, z), (x + 0.02, y + 0.02, z + 0.95), "Crew orange")
    # The railings round the opening, and a sign on the far one.
    _railing(p, (-ox - 0.12, -L + 0.05), (-ox - 0.12, oy1 + 0.12))
    _railing(p, (ox + 0.12, -L + 0.05), (ox + 0.12, oy1 + 0.12))
    _railing(p, (-ox - 0.12, oy1 + 0.12), (ox + 0.12, oy1 + 0.12))
    sy = oy1 + 0.12 - 0.03
    p.span((-0.45, sy - 0.012, 0.55), (0.45, sy, 0.85), "Crew orange")
    with p.painted():
        p.lettering("LOWER DECKS", (0.06, sy - 0.017, 0.75), 0.08, "Hull dark")
        p.prism([(-0.05, 0.02), (0.05, 0.02), (0.05, 0.0), (0.0, -0.06), (-0.05, 0.0)], 0.004,
                (-0.32, sy - 0.016, 0.7), "Hull dark")
        p.lettering(str(r.randint(2, 4)) + "-" + str(r.randint(10, 60)), (0.06, sy - 0.017, 0.62), 0.06, "Hull dark")
    with p.lowpoly():
        for k in range(WELL_STEPS - 1):
            z = -rise * (k + 1)
            ya, yb = y_top + tread * k, y_top + tread * (k + 1)
            p.span((-w + 0.08, ya, z - 0.05), (w - 0.08, yb, z), "Grating")
        for x in (-w, w - 0.08):
            p.box((0.08, length + 0.15, 0.3), (x + 0.04, mid_y, mid_z - 0.08), "Hull dark", rot=(-math.degrees(a), 0, 0))
        for x in (-w + 0.04, w - 0.04):
            p.box((0.05, length, 0.05), (x, mid_y, mid_z + 0.95), "Crew orange", rot=(-math.degrees(a), 0, 0))
        p.span((-0.45, sy - 0.012, 0.55), (0.45, sy, 0.85), "Crew orange")
    return p


# The lift ------------------------------------------------------------------

def freight_lift(style: Style) -> Piece:
    """A caged freight lift through the decks: a 2.4 m square platform at
    the floor's level, a tower of four I-beam posts from the deck below to
    above the deck over it, mesh on three sides, a folding gate at the
    front, a winch house on top with cables down to the platform, a
    counterweight in its guides, and a call box with its buttons."""
    p = Piece("freight_lift", "prop", "2.8 × 2.8 m lift tower, platform at 0, -3.6 to 4.6 m", budget=20000)
    r = rng(style, p.name)
    s = 1.3  # the posts' half spacing
    lo, hi = -DECK, DECK + 1.0
    paint = r.choice(["Crew orange", "Hazard yellow"])
    # The posts: I-beams, bolted foot plates, cross bracing on the back and
    # the sides between the decks.
    for sx in (-1, 1):
        for sy in (-1, 1):
            cx, cy = sx * s, sy * s
            p.span((cx - 0.1, cy - 0.1, lo), (cx + 0.1, cy - 0.08, hi), "Hull dark")
            p.span((cx - 0.1, cy + 0.08, lo), (cx + 0.1, cy + 0.1, hi), "Hull dark")
            p.span((cx - 0.012, cy - 0.08, lo), (cx + 0.012, cy + 0.08, hi), "Hull dark")
            p.span((cx - 0.16, cy - 0.16, lo), (cx + 0.16, cy + 0.16, lo + 0.02), "Hull dark")
            p.collider((0.2, 0.2, hi - lo), (cx, cy, (lo + hi) / 2))
    for z0, z1 in ((lo + 0.3, -0.4), (0.3, DECK - 0.2)):
        for side in ("back", "left", "right"):
            if side == "back":
                a0, a1 = (-s, s - 0.1, z0), (s, s - 0.1, z1)
                b0, b1 = (s, s - 0.1, z0), (-s, s - 0.1, z1)
            else:
                x = -s + 0.1 if side == "left" else s - 0.1
                a0, a1 = (x, -s, z0), (x, s, z1)
                b0, b1 = (x, s, z0), (x, -s, z1)
            p.tube([a0, a1], 0.022, "Crew grey", segments=4)
            p.tube([b0, b1], 0.022, "Crew grey", segments=4)
    # Ring beams at each deck and the top.
    # Ring beams between the posts at each deck and the top: centred on the
    # posts' lines and stopping at their flanges, so no face of one lies in
    # a face of the other.
    for z in (-0.25, DECK - 0.1, hi - 0.2):
        for sy in (-1, 1):
            p.span((-s + 0.1, sy * s - 0.05, z - 0.1), (s - 0.1, sy * s + 0.05, z + 0.1), paint)
        for sx in (-1, 1):
            p.span((sx * s - 0.05, -s + 0.1, z - 0.1), (sx * s + 0.05, s - 0.1, z + 0.1), paint)
    # The platform: a frame and tread plate, its edges painted, toe guards.
    q = 1.15
    p.span((-q, -q, -0.22), (q, q, -0.04), "Crew grey")
    _tread(p, r, -q, q, -q, q, "Hull alloy", paint=True)
    p.collider((2 * q, 2 * q, 0.22), (0, 0, -0.11))
    with p.painted():
        for k in range(12):
            if k % 2:
                continue
            x0 = -q + 2 * q * k / 12
            p.span((x0, -q - 0.005, -0.2), (x0 + 2 * q / 12, -q, -0.05), "Hazard yellow")
    # The cage on the platform: mesh panels (bars) on three sides, plain.
    cage_h = 2.3
    with p.plain():
        for side in ("back", "left", "right"):
            n = 15
            # Bars stop inside the top rail; the rails stand a few
            # millimetres proud of the bars both ways, so their faces never
            # share a plane.
            for k in range(1, n):  # not at the corners, where the rails meet
                t = -q + 2 * q * k / n
                if side == "back":
                    p.span((t - 0.006, q - 0.012, 0), (t + 0.006, q, cage_h - 0.012), "Steel")
                else:
                    x = -q if side == "left" else q - 0.012
                    p.span((x, t - 0.006, 0), (x + 0.012, t + 0.006, cage_h - 0.012), "Steel")
            for z in (0.05, 1.1, cage_h - 0.03):
                if side == "back":
                    p.span((-q + 0.03, q - 0.03, z), (q - 0.03, q + 0.006, z + 0.03), "Crew grey")
                else:
                    x = -q - 0.006 if side == "left" else q - 0.03
                    p.span((x, -q, z), (x + 0.036, q, z + 0.03), "Crew grey")
    with p.plain():  # as the rails it meets are
        p.span((-q, -q, cage_h - 0.03), (q, -q + 0.04, cage_h), "Crew grey")
    p.collider((2 * q, 0.06, cage_h), (0, q - 0.03, cage_h / 2))
    p.collider((0.06, 2 * q, cage_h), (-q + 0.03, 0, cage_h / 2))
    p.collider((0.06, 2 * q, cage_h), (q - 0.03, 0, cage_h / 2))
    # The front gate, folded back to one side: lattice bars.
    with p.plain():
        gx0, gx1 = -q + 0.05, -q + 0.4
        for k in range(6):
            x = gx0 + (gx1 - gx0) * k / 5
            p.span((x - 0.008, -q - 0.04, 0.02), (x + 0.008, -q - 0.025, cage_h - 0.1), "Steel")
        for z in (0.1, 0.7, 1.3, 1.9):
            p.box((0.45, 0.01, 0.015), ((gx0 + gx1) / 2, -q - 0.032, z + 0.2), "Steel", rot=(0, 35, 0))
            p.box((0.45, 0.01, 0.015), ((gx0 + gx1) / 2, -q - 0.032, z + 0.2), "Steel", rot=(0, -35, 0))
    # The winch house on top, its cables down to the platform's corners.
    p.span((-0.6, -0.5, hi - 0.1), (0.6, 0.5, hi + 0.55), paint)
    p.span((-0.62, -0.52, hi + 0.55), (0.62, 0.52, hi + 0.6), "Hull dark")
    p.cyl(0.18, 0.9, (0, 0.6, hi + 0.25), "Hull dark", rot=(0, 90, 0), segments=12)
    for x in (-0.3, 0.3):
        for y in (-0.3, 0.3):
            p.cable((x, y, hi - 0.1), (x * 3, y * 3, cage_h + 0.02), 0.01, "Steel", steps=1, segments=3)
    p.span((-q, -q, cage_h), (q, q, cage_h + 0.04), "Crew grey")
    # The counterweight in its guide on the back.
    p.span((-0.3, s + 0.12, 1.2), (0.3, s + 0.32, 2.0), "Hull dark")
    for x in (-0.36, 0.36):
        p.span((x - 0.02, s + 0.1, lo), (x + 0.02, s + 0.14, hi), "Steel")
    with p.painted():
        for k in range(5):
            p.box((0.1, 0.006, 0.22), (-0.24 + k * 0.12, s + 0.323, 1.6), "Hazard yellow", rot=(0, 35, 0))
    # The call box on the front post: buttons, a lamp, a stencil.
    bx, by = s + 0.13, -s
    p.span((bx - 0.03, by - 0.12, 1.1), (bx + 0.06, by + 0.12, 1.45), "Crew grey")
    for z, mat in ((1.36, "Status blue"), (1.28, "Medical red"), (1.2, "Repaint green")):
        p.cyl(0.022, 0.03, (bx + 0.07, by, z), mat, rot=(0, 90, 0), segments=8)
    p.cable((bx, by, 1.1), (bx - 0.02, by - 0.09, -0.3), 0.012, "Rubber", sag=0.05, steps=3, segments=4)
    with p.painted():
        p.stencil(str(r.randint(1, 9)), (0.5, -s - 0.005, DECK - 0.1), 0.14, "Stencil white")
        p.lettering("LIFT", (-0.3, -s - 0.005, DECK - 0.1), 0.12, "Stencil white")
    return p


# The vent -------------------------------------------------------------------

def shaft_grate(style: Style) -> Piece:
    """A grated cover over a ventilation shaft from the air plant below: a
    heavy bolted frame, bearing bars across, and under them a fan in its
    housing lit orange by the plant's sodium lamps below, the light fading
    up the shaft's sides. The game draws a slab with all of it, light and
    all, baked onto its top. Walked on."""
    p = Piece("shaft_grate", "kit", "2 × 2 m grated vent, top at 0", budget=500)
    p.collider((GRID, GRID, SLAB), (0, 0, -SLAB / 2))
    f = 0.16
    # The frame, bolted.
    for y0, y1 in ((-H, -H + f), (H - f, H)):
        p.span((-H, y0, -SLAB), (H, y1, 0), "Hull alloy")
    for x0, x1 in ((-H, -H + f), (H - f, H)):
        p.span((x0, -H + f, -SLAB), (x1, H - f, 0), "Hull alloy")
    for y in (-H + f / 2, H - f / 2):
        _bolts_along(p, (-H + 0.12, y, 0), (H - 0.12, y, 0), 0.22, "+Z")
    for x in (-H + f / 2, H - f / 2):
        _bolts_along(p, (x, -H + 0.3, 0), (x, H - 0.3, 0), 0.22, "+Z")
    # The grating: deep bearing bars, a cross bar.
    g = H - f
    y = -g + 0.03
    while y < g - 0.02:
        p.span((-g, y - 0.007, -0.06), (g, y + 0.007, -0.003), "Grating")
        y += 0.05
    p.span((-0.012, -g, -0.07), (0.012, g, -0.009), "Hull dark")
    # Under it, in reach of the bake (it reaches about 13 cm under a 2 m
    # stand-in's top): the shaft's walls fading from dark under the grating
    # to the lamps' orange glow, a fan's hub and blades, and the glowing
    # plant behind them.
    for k, (z, mat) in enumerate(((-0.066, "Hull dark"), (-0.084, "Rust"), (-0.102, "Crew orange"))):
        p.span((-g, -g, z - 0.018), (-g + 0.01, g, z), mat)
        p.span((g - 0.01, -g, z - 0.018), (g, g, z), mat)
        p.span((-g + 0.01, -g, z - 0.018), (g - 0.01, -g + 0.01, z), mat)
        p.span((-g + 0.01, g - 0.01, z - 0.018), (g - 0.01, g, z), mat)
    p.span((-g, -g, -0.128), (g, g, -0.12), "Sodium lamp")
    p.cyl(0.1, 0.03, (0, 0, -0.085), "Hull dark", segments=10)
    for k in range(5):
        a = 72 * k
        p.box((0.6, 0.12, 0.01), (math.cos(math.radians(a)) * 0.35, math.sin(math.radians(a)) * 0.35, -0.09),
              "Hull dark", rot=(18, 0, a))
    for x in (-0.6, 0.6):
        p.span((x - 0.015, -g + 0.01, -0.118), (x + 0.015, g - 0.01, -0.1), "Hull dark")
    with p.painted():
        p.stencil("41", (-0.75, -H - 0.005, -0.15), 0.1, "Stencil white")
    with p.lowpoly():
        p.span((-H, -H, -SLAB), (H, H, 0), "Hull dark")
    return p


PIECES = [deck_ceiling, deck_hatch, stairwell, freight_lift, shaft_grate]
