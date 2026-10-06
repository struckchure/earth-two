"""The props: real objects, retrofitted (docs/look-and-feel.md). Crates and
drums from the Pads, the Exchange's kiosks and its public board, the
market's stalls and their fabric, the lamps the Hull works by. What's
carried (filters, rebreathers and the rest) is in items.py.

Like the kit, each stands on its origin, its front looking along -Y."""
import math

from kit import FABRICS, PALETTE, REPAINTS, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "The Hull market"

PALETTE.setdefault("Ink", (0.01, 0.01, 0.03))


def _bolt(p: Piece, at, rot, mat: str = "Steel", r: float = 0.01, proud: float = 0.005) -> None:
    """A bolt head on a face: at is on the face, rot turns a Z cylinder out
    of it."""
    p.cyl(r, 2 * proud, at, mat, rot=rot, segments=4)


FRONT, BACK, LEFT, RIGHT, UP = (90, 0, 0), (-90, 0, 0), (0, -90, 0), (0, 90, 0), (0, 0, 0)


# Crates and drums -------------------------------------------------------------

def _crate(p: Piece, style: Style, size, number: str) -> Piece:
    """A Crew freight crate: a steel frame with corner castings round
    corrugated panels, a recessed lid with lifting lugs, handles at the
    ends, clasps, skids for the forks, and its number stencilled on."""
    sx, sy, h = size
    x, y = sx / 2, sy / 2
    p.collider(size, (0, 0, h / 2))
    r = rng(style, p.name)
    body = "Crew orange" if r.random() > style.repaint * 0.6 else r.choice(REPAINTS)
    frame, dark = "Crew grey", "Hull dark"
    skid, base = 0.06, 0.12
    # Skids under it, for the forks: two runners.
    for yy in (-y + 0.12, y - 0.22):
        p.span((-x + 0.04, yy, 0), (x - 0.04, yy + 0.1, skid), dark)
    # The body: panels inset from the frame.
    p.span((-x + 0.03, -y + 0.03, base), (x - 0.03, y - 0.03, h - 0.07), body)
    # The frame: a base rim, a top rim and corner posts.
    e = 0.07
    for z0, z1 in ((skid, base), (h - 0.07, h)):
        p.span((-x, -y, z0), (x, -y + e, z1), frame)
        p.span((-x, y - e, z0), (x, y, z1), frame)
        p.span((-x, -y + e, z0), (-x + e, y - e, z1), frame)
        p.span((x - e, -y + e, z0), (x, y - e, z1), frame)
    for cx in (-x, x - e):
        for cy in (-y, y - e):
            p.span((cx, cy, skid), (cx + e, cy + e, h), frame)
    # Corner castings at the eight corners, with their holes painted dark.
    c = 0.1
    for cx in (-x - 0.005, x - c + 0.005):
        for cy in (-y - 0.005, y - c + 0.005):
            for cz in (skid - 0.005, h - c + 0.005):
                p.span((cx, cy, cz), (cx + c, cy + c, cz + c), dark)
    # Corrugation: ribs pressed out of the long faces, a band left plain for
    # the stencil, and down the ends.
    for side in (-1, 1):
        face = side * (y - 0.03)
        k = -x + 0.16
        while k < x - 0.12:
            if abs(k) > 0.24:
                p.span((k - 0.025, min(face, face + side * 0.018), base + 0.04),
                       (k + 0.025, max(face, face + side * 0.018), h - 0.11), body)
            k += 0.12
        endface = side * (x - 0.03)
        k = -y + 0.15
        while k < y - 0.12:
            p.span((min(endface, endface + side * 0.018), k - 0.025, base + 0.04),
                   (max(endface, endface + side * 0.018), k + 0.025, h - 0.11), body)
            k += 0.12
    # The lid: a panel set down in the top rim, two lifting lugs and a hatch.
    p.span((-x + e, -y + e, h - 0.07), (x - e, y - e, h - 0.035), body)
    for lx in (-x / 2, x / 2):
        p.torus(0.045, 0.01, (lx, 0, h - 0.035), "Steel", rot=(90, 0, 0), segments=4, sides=2, arc=180)
        p.span((lx - 0.06, -0.03, h - 0.035), (lx + 0.06, 0.03, h - 0.029), "Steel")
    p.span((-0.2, -y + 0.15, h - 0.035), (0.2, -y + 0.38, h - 0.027), body)
    # Handles at the ends, folded down flat on their mounts (inside the
    # frame's line, so crates stand side by side).
    for side in (-1, 1):
        ex = side * (x - 0.03 + 0.018)
        for hy in (-0.12, 0.12):
            p.span((min(ex, ex + side * 0.012), hy - 0.02, h * 0.62 - 0.03),
                   (max(ex, ex + side * 0.012), hy + 0.02, h * 0.62 + 0.03), dark)
        hx = ex + side * 0.012
        p.tube([(hx, -0.12, h * 0.62), (hx, -0.1, h * 0.62 - 0.07), (hx, 0.1, h * 0.62 - 0.07), (hx, 0.12, h * 0.62)],
               0.011, "Steel", segments=3)
    # Clasps where the lid meets the front, and their hinges at the back.
    for cxs in (-x / 2, x / 2):
        p.span((cxs - 0.04, -y - 0.012, h - 0.16), (cxs + 0.04, -y + 0.01, h - 0.05), "Steel")
        p.span((cxs - 0.015, -y - 0.022, h - 0.13), (cxs + 0.015, -y - 0.012, h - 0.08), dark)
        p.cyl(0.015, 0.14, (cxs, y + 0.005, h - 0.04), "Steel", rot=(0, 90, 0), segments=3)
    # Now and then a plate welded over a dent on an end.
    if r.random() < 0.5:
        side = r.choice((-1, 1))
        ex = side * (x - 0.03 + 0.025)
        p.span((min(ex, ex + side * 0.008), -0.15, base + 0.08), (max(ex, ex + side * 0.008), 0.12, base + 0.3),
               r.choice(REPAINTS))
    with p.painted():
        face = y - 0.03 + 0.006
        p.stencil(number, (0, -face, h - 0.28), 0.2, "Stencil white")
        p.stencil(number, (0, face, h - 0.28), 0.2, "Stencil white", facing="+Y")
        p.span((-0.2, -face - 0.004, h - 0.5), (0.2, -face, h - 0.46), "Stencil white")
        # A manifest label on the front, its lines in ink.
        p.span((-0.17, -face - 0.004, base + 0.08), (0.17, -face, base + 0.28), "Paper")
        for i in range(4):
            p.span((-0.15, -face - 0.0095, base + 0.12 + i * 0.04),
                   (0.15 - 0.06 * (i % 2), -face - 0.0055, base + 0.135 + i * 0.04), "Ink")
        # Casting holes.
        for cx in (-x + c / 2 - 0.005, x - c / 2 + 0.005):
            for cz in (skid + c / 2 - 0.005, h - c / 2 + 0.005):
                p.span((cx - 0.025, -y - 0.012, cz - 0.015), (cx + 0.025, -y - 0.008, cz + 0.015), "Rubber")
    return p


def crate(style: Style) -> Piece:
    """1.2 × 0.9 × 0.9 m: vaulted from its long side (the clip's box is 0.89 m)."""
    return _crate(Piece("crate", "prop", "1.2 × 0.9 × 0.9 m: vaultable"), style, (1.2, 0.9, 0.9), "42")


def crate_tall(style: Style) -> Piece:
    """1.2 × 0.9 × 1.6 m: too high to vault, low enough to mantle."""
    return _crate(Piece("crate_tall", "prop", "1.2 × 0.9 × 1.6 m: mantle"), style, (1.2, 0.9, 1.6), "117")


def drum(style: Style) -> Piece:
    """A 200 litre steel drum: Poly Haven's barrel 03, dented and scuffed,
    as often as not painted over in whoever's colour and numbered."""
    r = rng(style, "drum")
    paint = r.choice([None, None] + REPAINTS + ["Crew orange", "Crew orange"])
    p = Sourced("drum", "prop", "0.6 m across, 0.9 m high: vaultable", asset="barrel_03", height=0.9,
                recolour=paint)
    p.collider((0.6, 0.6, 0.9), (0, 0, 0.45))
    with p.painted():
        p.stencil(str(r.randint(10, 99)), (0, -0.312, 0.52), 0.12, "Stencil white")
    return p


# The Exchange's furniture in the market -------------------------------------------

def terminal_kiosk(style: Style) -> Piece:
    """An Exchange terminal: a white pillar on a bolted plinth, a hooded
    screen, a keypad and card slot on a sloped shelf, a receipt slot, the
    Registrar's brass seal, and a stamp on a chain."""
    p = Piece("terminal_kiosk", "prop", "0.7 m wide, 1.75 m high")
    p.collider((0.6, 0.5, 1.75), (0, 0, 0.875))
    # The plinth, bolted down.
    p.span((-0.3, -0.25, 0), (0.3, 0.25, 0.06), "Hull dark")
    p.span((-0.24, -0.19, 0.06), (0.24, 0.19, 0.1), "Hull alloy")
    for bx in (-0.27, 0.27):
        for by in (-0.22, 0.22):
            _bolt(p, (bx, by, 0.06), UP, r=0.012)
    # The pillar, with a service door and a vent.
    p.span((-0.16, -0.12, 0.1), (0.16, 0.14, 1.14), "Charter white")
    p.span((-0.12, -0.132, 0.2), (0.12, -0.12, 0.8), "Charter white")
    for hz in (0.28, 0.72):
        p.cyl(0.008, 0.05, (-0.125, -0.13, hz), "Steel", segments=4)
    p.cyl(0.014, 0.01, (0.09, -0.135, 0.5), "Brass", rot=FRONT, segments=4)
    for i in range(5):
        p.span((0.16, -0.05 + i * 0.03, 0.85), (0.168, -0.04 + i * 0.03, 1.0), "Hull dark")
    # The cable up the back, into the pillar.
    p.cable((0, 0.25, 0.03), (0, 0.15, 0.6), 0.018, "Rubber", sag=-0.05, segments=4, steps=4)
    # The shelf: sloped towards the user, with the keypad and card reader.
    def shelf(size, at, mat):
        # A box in the shelf's frame: at is (across, out, up) from its middle.
        c, sn = math.cos(math.radians(20)), math.sin(math.radians(20))
        x, y, u = at
        p.box(size, (x, -0.2 + y * c - u * sn, 1.06 + y * sn + u * c), mat, rot=(20, 0, 0))

    shelf((0.52, 0.26, 0.05), (0, 0, 0), "Hull dark")
    shelf((0.46, 0.2, 0.012), (0, 0, 0.029), "Hull alloy")
    for i in range(4):
        for j in range(3):
            shelf((0.04, 0.035, 0.012), (-0.18 + j * 0.05, -0.07 + i * 0.045, 0.041), "Steel")
    shelf((0.12, 0.05, 0.02), (0.13, 0.0, 0.045), "Rubber")
    # The receipt slot under it, a slip hanging out.
    p.span((0.09, -0.14, 0.93), (0.15, -0.12, 0.96), "Rubber")
    p.span((0.105, -0.147, 0.86), (0.135, -0.14, 0.94), "Paper")
    # The head: a housing tilted back under a hood, a bezel round the screen
    # set back in it.
    def head(size, at, mat):
        c, sn = math.cos(math.radians(-12)), math.sin(math.radians(-12))
        x, y, u = at
        p.box(size, (x, y * c - u * sn, 1.45 + y * sn + u * c), mat, rot=(-12, 0, 0))

    head((0.7, 0.18, 0.5), (0, 0, 0), "Charter white")
    head((0.76, 0.26, 0.05), (0, -0.05, 0.27), "Hull dark")
    for sx in (-1, 1):
        head((0.03, 0.22, 0.48), (sx * 0.365, -0.05, 0.01), "Hull dark")
    for u in (-0.185, 0.185):
        head((0.62, 0.03, 0.03), (0, -0.1, u), "Hull dark")
    for xx in (-0.295, 0.295):
        head((0.03, 0.03, 0.4), (xx, -0.1, 0), "Hull dark")
    head((0.56, 0.01, 0.34), (0, -0.095, 0), "Status blue")
    # The seal on the pillar, and the stamp on its chain.
    p.cyl(0.07, 0.012, (0, -0.126, 0.95), "Brass", rot=FRONT, segments=10)
    p.cable((-0.24, -0.27, 1.06), (-0.28, -0.3, 0.9), 0.006, "Steel", sag=0.04, segments=2, steps=3)
    p.cyl(0.025, 0.04, (-0.28, -0.3, 0.88), "Wood dark", segments=6)
    p.cyl(0.03, 0.025, (-0.28, -0.3, 0.85), "Brass", segments=6)
    with p.painted():
        # The screen's lines of text, and the seal's ring.
        for i in range(5):
            w = 0.42 - 0.12 * (i % 2) - 0.05 * (i % 3)
            head((w, 0.004, 0.025), (-0.22 + w / 2, -0.106, 0.12 - i * 0.055), "Screen")
        p.cyl(0.055, 0.004, (0, -0.138, 0.95), "Hull dark", rot=FRONT, segments=10)
        p.cyl(0.04, 0.004, (0, -0.143, 0.95), "Brass", rot=FRONT, segments=10)
        p.stencil("12", (0, -0.138, 0.35), 0.1, "Hull dark")
    return p


# The market -----------------------------------------------------------------

def market_stall(style: Style) -> Piece:
    """A market stall for a 2 m bay: a counter of salvaged panels 0.9 m high
    (vaultable) piled with fruit, roots and pots, a back shelf of goods, a
    striped awning sagging between four pipe poles, and a string of lamps."""
    p = Piece("market_stall", "prop", "2 × 2 m stall, counter 0.9 m high, awning to 2.5 m")
    p.budget = 30000
    r = rng(style, p.name)
    main, stripe = FABRICS[style.fabric]
    w = 0.95
    # The counter: panels of whatever was going, riveted to a frame.
    p.collider((2 * w + 0.06, 0.58, 0.9), (0, -0.71, 0.45))
    p.span((-w, -0.92, 0), (w, -0.45, 0.86), "Hull dark")
    panels = [r.choice(REPAINTS) for _ in range(3)]
    for i, mat in enumerate(panels):
        x0 = -w + 0.02 + i * (2 * w - 0.04) / 3
        x1 = x0 + (2 * w - 0.04) / 3 - 0.015
        p.span((x0, -0.95, 0.04), (x1, -0.92, 0.84), mat)
        for zz in (0.08, 0.8):
            for k in range(5):
                _bolt(p, (x0 + 0.04 + k * (x1 - x0 - 0.08) / 4, -0.95, zz), FRONT, "Hull dark", r=0.008, proud=0.006)
    p.span((-w - 0.04, -1.0, 0.86), (w + 0.04, -0.42, 0.9), "Wood")
    # Fruit, roots and pots on the counter.
    p.source("bananas", at=(-0.62, -0.72, 0.9), length=0.32)
    p.source("plastic_crate_02", at=(0.45, -0.72, 0.9), length=0.42)
    for ax in (0.33, 0.45, 0.57):
        p.source("food_apple_01", at=(ax, -0.76, 1.12), height=0.08)
        p.source("lemon", at=(ax, -0.66, 1.12), height=0.07)
    for ax in (-0.25, -0.13):
        p.source("yellow_onion", at=(ax, -0.62, 0.9), height=0.08)
        p.source("sweet_potato", at=(ax - 0.02, -0.8, 0.9), length=0.18)
    p.source("pot_enamel_01", at=(0.5, 0.72, 1.03), height=0.2)
    p.source("brass_pot_01", at=(-0.5, 0.72, 1.53), height=0.2)
    # The back shelf: a steel frame of shelves, stocked.
    p.collider((2 * w, 0.35, 1.6), (0, 0.725, 0.8))
    for x in (-w, -0.02, w - 0.04):
        for y in (0.55, 0.86):
            p.span((x, y, 0), (x + 0.04, y + 0.04, 1.6), "Hull dark")
    for z in (0.15, 0.5, 1.0, 1.5):
        p.span((-w + 0.04, 0.56, z), (w - 0.04, 0.89, z + 0.03), "Hull alloy")
    for z in (0.18, 0.53):
        for i in range(7):
            x = -0.85 + i * 0.25 + r.uniform(-0.02, 0.02)
            if abs(x + 0.02) < 0.13:
                continue
            hh = r.uniform(0.12, 0.28)
            if r.random() < 0.5:
                p.span((x, 0.6, z), (x + 0.18, 0.84, z + hh), r.choice(REPAINTS + ["Paper aged", "Fabric ochre"]))
            else:
                for t in range(3):
                    p.cyl(0.03, hh * 0.7, (x + 0.03 + t * 0.06, 0.7, z + hh * 0.35),
                          r.choice(["Steel", "Crew orange", "Repaint teal", "Copper"]), segments=6)
    for i in range(6):
        p.cyl(0.04, 0.155, (-0.85 + i * 0.1, 0.72, 1.03 + 0.0775), "Fabric sand", segments=6)
        p.cyl(0.046, 0.04, (-0.85 + i * 0.1, 0.72, 1.175), "Crew orange", segments=6)
    # Poles: pipes on base plates, clamped to the awning's frame.
    for x, y, top in ((-w, -0.98, 2.2), (w, -0.98, 2.2), (-w, 0.95, 2.5), (w, 0.95, 2.5)):
        p.cyl(0.03, top - 0.012, (x, y, 0.012 + (top - 0.012) / 2), "Hull dark", segments=6)
        p.span((x - 0.07, y - 0.07, 0), (x + 0.07, y + 0.07, 0.012), "Hull dark")
        p.cyl(0.045, 0.06, (x, y, top - 0.05), "Steel", segments=6)
    for y, top in ((-0.98, 2.2), (0.95, 2.5)):
        p.cyl(0.022, 2 * w + 0.1, (0, y, top), "Hull dark", rot=(0, 90, 0), segments=6)
    # The awning: strips of the two fabrics sagging between the frame bars.
    n = 8
    x0 = -w - 0.1
    sw = (2 * w + 0.2) / n
    for i in range(n):
        xa, xb = x0 + i * sw, x0 + (i + 1) * sw
        p.cloth([(xa, -1.06, 2.25), (xb, -1.06, 2.25), (xb, 1.0, 2.55), (xa, 1.0, 2.55)],
                main if i % 2 == 0 else stripe, sag=0.1, ripple=0.01, droop=(0, 1, 0, 1), seed=3)
    # The valance: a scalloped flap along the front.
    for i in range(n):
        xa = x0 + i * sw
        mat = stripe if i % 2 == 0 else main
        p.box((sw, 0.012, 0.2), (xa + sw / 2, -1.068, 2.13), mat, rot=(4, 0, 0))
        p.cyl(sw / 2, 0.012, (xa + sw / 2, -1.064, 2.03), mat, rot=FRONT, segments=6)
    # A string of lamps along the front.
    p.cable((-w, -1.0, 2.18), (w, -1.0, 2.18), 0.006, "Rubber", sag=0.18, steps=6, segments=2)
    for i in range(1, 6):
        t = i / 6
        x = -w + 2 * w * t
        z = 2.18 - 0.18 * 4 * t * (1 - t)
        p.cyl(0.012, 0.03, (x, -1.0, z - 0.02), "Hull dark", segments=4)
        p.sphere(0.03, (x, -1.0, z - 0.06), "Sodium lamp", segments=6, rings=4)
    # A price board hung on the front pole.
    p.span((-w + 0.04, -1.055, 1.24), (-w + 0.41, -1.035, 1.56), "Wood")
    p.cable((-w + 0.02, -1.0, 1.75), (-w + 0.08, -1.04, 1.56), 0.004, "Fabric sand", segments=2, steps=1)
    with p.painted():
        p.span((-w + 0.06, -1.061, 1.26), (-w + 0.39, -1.057, 1.54), "Ink")
        p.stencil("5", (-w + 0.14, -1.067, 1.43), 0.1, "Stencil white")
        p.stencil("12", (-w + 0.3, -1.067, 1.33), 0.1, "Stencil white")
        p.stencil(str(r.randint(10, 99)), (0, -0.957, 0.6), 0.16, "Stencil white")
    return p


def tarp_awning(style: Style) -> Piece:
    """A lean-to of tarp: hung from a ridge pole on two uprights, its back
    edge tied to two short poles, guyed out to pegs, sandbags on the feet.
    Shade, or a bit of privacy."""
    p = Piece("tarp_awning", "prop", "3 × 2 m tarp, 2.4 m at the front")
    p.budget = 30000  # the cloth, both faces, rippled
    main = FABRICS[style.fabric][0]
    front, back = 2.4, 1.6
    p.cloth([(-1.5, -1.0, front + 0.04), (1.5, -1.0, front + 0.04), (1.5, 0.95, back + 0.04), (-1.5, 0.95, back + 0.04)],
            main, sag=0.16, ripple=0.018, droop=(0.4, 1, 0.4, 1), seed=11)
    for x, y, top in ((-1.4, -1.0, front), (1.4, -1.0, front), (-1.4, 0.95, back), (1.4, 0.95, back)):
        p.cyl(0.025, top, (x, y, top / 2), "Wood dark", segments=6)
        # A sandbag on its foot.
        p.sphere(0.13, (x + 0.22, y, 0.06), "Fabric sand", scale=(1.3, 0.8, 0.5), segments=8, rings=5)
    p.cyl(0.03, 3.1, (0, -1.0, front), "Wood dark", rot=(0, 90, 0), segments=6)
    p.cyl(0.025, 3.0, (0, 0.95, back), "Wood dark", rot=(0, 90, 0), segments=6)
    # Guy ropes out to pegs.
    for x in (-1.4, 1.4):
        peg = (x * 1.2, -1.85, 0.0)
        p.cable((x, -1.0, front), peg, 0.006, "Canvas", sag=0.05, segments=2, steps=3)
        p.cyl(0.012, 0.18, (peg[0], peg[1], 0.06), "Steel", rot=(25, 0, 0), segments=4)
    return p


def work_lamp(style: Style) -> Piece:
    """A sodium work lamp on a telescopic tripod: the Hull's orange light.
    A finned housing behind a caged lens, on a yoke, its cable trailing to
    the floor."""
    p = Piece("work_lamp", "prop", "1.9 m lamp stand; no collider")
    hub = 0.85
    for i in range(3):
        a = math.tau * i / 3 + 0.5
        foot = (math.cos(a) * 0.45, math.sin(a) * 0.45, 0.03)
        p.tube([(0, 0, hub), foot], 0.014, "Hull dark", segments=4)
        p.cyl(0.025, 0.03, (foot[0], foot[1], 0.015), "Rubber", segments=6)
        mid = (math.cos(a) * 0.22, math.sin(a) * 0.22, hub * 0.5)
        p.tube([(0, 0, 0.55), mid], 0.008, "Hull dark", segments=3)
    p.cyl(0.035, 0.1, (0, 0, hub), "Crew orange", segments=6)
    p.cyl(0.022, 0.6, (0, 0, 0.55 + 0.3), "Hull dark", segments=6)
    p.cyl(0.03, 0.06, (0, 0, 1.15), "Crew orange", segments=6)
    p.cyl(0.012, 0.06, (0.05, 0, 1.15), "Hull dark", rot=RIGHT, segments=4)
    p.cyl(0.016, 0.5, (0, 0, 1.15 + 0.25), "Steel", segments=6)
    top = 1.68
    # The yoke, and the head on it, tilted down.
    p.tube([(0, 0, top - 0.06), (-0.22, 0, top - 0.06), (-0.22, 0, top + 0.12)], 0.012, "Crew orange", segments=4)
    p.tube([(0, 0, top - 0.06), (0.22, 0, top - 0.06), (0.22, 0, top + 0.12)], 0.012, "Crew orange", segments=4)
    p.cyl(0.03, 0.05, (0, 0, top - 0.06), "Crew orange", segments=6)
    for sx in (-1, 1):
        p.cyl(0.03, 0.04, (sx * 0.24, 0, top + 0.1), "Hull dark", rot=RIGHT, segments=6)
    tilt = -20
    head = (0, 0, top + 0.1)

    def hb(size, off, mat):
        # A box in the head's frame, tilted with it.
        c, s = math.cos(math.radians(tilt)), math.sin(math.radians(tilt))
        x, y, z = off
        p.box(size, (head[0] + x, head[1] + y * c - z * s, head[2] + y * s + z * c), mat, rot=(tilt, 0, 0))

    hb((0.4, 0.2, 0.3), (0, 0.02, 0), "Crew orange")
    for i in range(7):
        hb((0.36, 0.06, 0.012), (0, 0.15, -0.12 + i * 0.04), "Hull dark")
    hb((0.42, 0.03, 0.32), (0, -0.09, 0), "Hull dark")
    hb((0.33, 0.012, 0.23), (0, -0.106, 0), "Sodium lamp")
    p.light((0, -0.4, top - 0.05), intensity=1.3, reach=8)
    for i in range(4):
        hb((0.01, 0.02, 0.3), (-0.12 + i * 0.08, -0.13, 0), "Steel")
    hb((0.22, 0.03, 0.03), (0, 0.02, 0.17), "Hull dark")
    # Its cable down the mast to the floor, and away.
    p.cable((0.05, 0.12, top + 0.05), (0.05, 0.06, 0.9), 0.008, "Rubber", sag=-0.04, segments=2, steps=3)
    p.cable((0.05, 0.06, 0.9), (0.6, 0.5, 0.008), 0.008, "Rubber", sag=0.2, segments=2, steps=4)
    p.cable((0.6, 0.5, 0.008), (1.0, 0.4, 0.008), 0.008, "Rubber", segments=2, steps=1)
    return p


def status_light(style: Style) -> Piece:
    """A wall-mounted status light: a cold blue dome behind a wire cage, on
    a housing bolted to the wall, its conduit going up. Its back goes on a
    wall's face."""
    p = Piece("status_light", "prop", "0.2 m wall light; no collider")
    p.span((-0.11, -0.012, -0.16), (0.11, 0, 0.16), "Hull dark")
    for bx in (-0.09, 0.09):
        for bz in (-0.14, 0.14):
            _bolt(p, (bx, -0.012, bz), FRONT, r=0.008, proud=0.006)
    p.span((-0.08, -0.07, -0.1), (0.08, -0.012, 0.1), "Crew grey")
    p.lathe([(0.0, 0.0), (0.06, 0.0), (0.058, 0.02), (0.045, 0.045), (0.02, 0.06), (0.0, 0.063)], (0, -0.07, 0.02),
            "Status blue", rot=(90, 0, 0), segments=10)
    p.light((0, -0.2, 0.02), "Status blue", intensity=0.8, reach=2.5)
    p.torus(0.064, 0.004, (0, -0.1, 0.02), "Steel", rot=(90, 0, 0), segments=8, sides=2)
    p.torus(0.045, 0.004, (0, -0.125, 0.02), "Steel", rot=(90, 0, 0), segments=8, sides=2)
    for i in range(4):
        a = math.tau * (i + 0.5) / 4
        p.tube([(math.cos(a) * 0.064, -0.1, 0.02 + math.sin(a) * 0.064),
                (math.cos(a) * 0.045, -0.125, 0.02 + math.sin(a) * 0.045)], 0.004, "Steel", segments=2)
    p.cyl(0.015, 0.12, (0, -0.04, 0.16), "Hull dark", segments=6)
    with p.painted():
        p.span((-0.06, -0.076, -0.09), (0.06, -0.072, -0.06), "Stencil white")
        p.stencil(str(rng(style, p.name).randint(1, 9)), (0, -0.08, -0.075), 0.022, "Hull dark")
    return p


def exchange_board(style: Style) -> Piece:
    """The Exchange's public board: contracts pinned up in rows under a brass
    header, two lamps on goosenecks over them, a tally of filings chalked
    along the foot, and the floor bell that rings when a big one is filed."""
    p = Piece("exchange_board", "prop", "3 m wide public board, 2.9 m high, with the floor bell")
    r = rng(style, p.name)
    w = 1.5
    p.collider((2 * w, 0.2, 2.9), (0, 0, 1.45))
    # Posts: I-beams on bolted feet.
    for x in (-w + 0.06, w - 0.06):
        p.span((x - 0.06, -0.08, 0.04), (x + 0.06, -0.065, 2.5), "Hull dark")
        p.span((x - 0.06, 0.065, 0.04), (x + 0.06, 0.08, 2.5), "Hull dark")
        p.span((x - 0.01, -0.065, 0.04), (x + 0.01, 0.065, 2.5), "Hull dark")
        p.span((x - 0.12, -0.12, 0), (x + 0.12, 0.12, 0.04), "Hull alloy")
        for bx in (-0.09, 0.09):
            for by in (-0.095, 0.095):
                _bolt(p, (x + bx, by, 0.04), UP, r=0.012)
    # The board: navy, framed in dark steel.
    p.span((-w + 0.12, -0.05, 0.45), (w - 0.12, 0.05, 2.5), "Hull dark")
    p.span((-w + 0.17, -0.06, 0.5), (w - 0.17, -0.05, 2.45), "Repaint navy")
    # The header: brass, a dark band set in it.
    p.span((-w - 0.05, -0.1, 2.5), (w + 0.05, 0.08, 2.75), "Brass")
    p.span((-w + 0.2, -0.11, 2.56), (w - 0.2, -0.1, 2.69), "Hull dark")
    # Lamps on goosenecks over the board.
    for x in (-0.75, 0.75):
        p.tube([(x, -0.1, 2.72), (x, -0.25, 2.85), (x, -0.4, 2.8)], 0.012, "Hull dark", segments=4)
        p.cyl(0.08, 0.08, (x, -0.42, 2.76), "Hull dark", radius2=0.03, segments=8)
        p.cyl(0.07, 0.01, (x, -0.42, 2.71), "Sodium lamp", segments=8)
    p.light((0, -0.6, 2.5), intensity=0.8, reach=4)
    # The contracts: slips in columns, pinned, some stamped.
    slips = []
    for c in range(9):
        for row in range(5):
            if r.random() < 0.15:
                continue
            x = -w + 0.25 + c * 0.295 + r.uniform(-0.02, 0.02)
            z = 0.6 + row * 0.36 + r.uniform(-0.02, 0.02)
            slips.append((x, z, r.random() < 0.4, r.random()))
    with p.painted():
        for x, z, stamped, k in slips:
            p.span((x, -0.068, z), (x + 0.22, -0.064, z + 0.3), "Paper" if k > 0.3 else "Paper aged")
            for i in range(5):
                p.span((x + 0.02, -0.0745, z + 0.2 - i * 0.035), (x + 0.2 - 0.05 * (i % 2), -0.0715, z + 0.212 - i * 0.035), "Ink")
            if stamped:
                p.span((x + 0.12, -0.0745, z + 0.02), (x + 0.2, -0.0715, z + 0.06), r.choice(["Wax red", "Brass"]))
            # Its pin.
            p.span((x + 0.1, -0.0745, z + 0.265), (x + 0.12, -0.0715, z + 0.285), "Brass")
        p.stencil("41", (0, -0.116, 2.625), 0.09, "Brass")
        # The tally, chalked along the foot.
        p.span((-w + 0.2, -0.066, 0.455), (w - 0.2, -0.062, 0.495), "Ink")
        for i in range(24):
            p.span((-w + 0.25 + i * 0.1, -0.0725, 0.46), (-w + 0.26 + i * 0.1, -0.0695, 0.49), "Stencil white")
    # The bell, on a bracket off the right post, its rope hanging.
    p.span((w, -0.03, 2.2), (w + 0.45, 0.03, 2.26), "Hull dark")
    p.box((0.04, 0.04, 0.3), (w + 0.1, 0, 2.08), "Hull dark", rot=(0, 45, 0))
    p.cyl(0.02, 0.06, (w + 0.35, 0, 2.18), "Hull dark", segments=4)
    p.lathe([(0.0, 0.24), (0.03, 0.24), (0.06, 0.23), (0.075, 0.17), (0.09, 0.08), (0.13, 0.02), (0.145, 0.0),
             (0.0, 0.0)], (w + 0.35, 0, 1.91), "Brass", segments=12)
    p.cable((w + 0.35, 0, 1.92), (w + 0.37, -0.02, 1.25), 0.008, "Canvas", segments=2, steps=1)
    p.sphere(0.025, (w + 0.37, -0.02, 1.24), "Canvas", segments=6, rings=4)
    return p


PIECES = [crate, crate_tall, drum, terminal_kiosk, market_stall, tarp_awning, work_lamp, status_light, exchange_board]
