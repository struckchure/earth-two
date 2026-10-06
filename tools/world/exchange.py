"""The Exchange: its floor fills the Hull's old cargo bay (docs/exchange.md).
Rows of terminals, the public board (exchange_board, in props.py), and the
Registrars behind their counters: the colony's only institution, so worn
ship parts made dignified with dark paint, brass and paperwork.

Like the Hull kit, each piece stands on its origin, its front looking along
-Y: the public's side. The Registrars work from +Y. Real furniture (the desks,
the bench, the lamps, the chairs) is Poly Haven's (see polyhaven.py); the
rest is modelled, its paint (labels, numbers, stripes) baked in."""
import math

from mathutils import Matrix, Vector

from kit import DECK, PALETTE, REPAINTS, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "The Exchange"

PALETTE.setdefault("Leather green", (0.04, 0.12, 0.06))
PALETTE.setdefault("Ink", (0.01, 0.01, 0.03))

PAPERWORK = ["Paper", "Paper aged", "Repaint cream"]
BINDINGS = ["Leather", "Charter navy", "Fabric red", "Repaint green", "Repaint navy"]


# Shared parts ----------------------------------------------------------------

def _papers(p: Piece, r, at, n: int, size=(0.21, 0.3)) -> None:
    """A stack of n loose sheets, each a little askew, with a gap between
    them so no two share a plane."""
    x, y, z = at
    for i in range(n):
        p.box((size[0], size[1], 0.003), (x + r.uniform(-0.012, 0.012), y + r.uniform(-0.012, 0.012), z + 0.0065 + i * 0.006),
              r.choice(PAPERWORK), rot=(0, 0, r.uniform(-7, 7)))
    # The top sheet's lines of type, painted on it.
    top = z + 0.0065 + (n - 1) * 0.006 + 0.0015 + 0.006
    with p.painted():
        for k in range(6):
            p.box((size[0] * (0.7 if k % 3 else 0.45), 0.008, 0.002), (x - 0.01, y + size[1] * 0.35 - k * 0.035, top), "Ink")


def _ledgers(p: Piece, r, at, n: int) -> None:
    """A pile of bound ledgers: a cover round paper edges, banded spines."""
    x, y, z = at
    for i in range(n):
        a = r.uniform(-8, 8)
        h = r.uniform(0.035, 0.05)
        cover = r.choice(BINDINGS)
        p.box((0.32, 0.24, h), (x, y, z + h / 2), cover, rot=(0, 0, a))
        p.box((0.305, 0.25, h * 0.72), (x + 0.012, y, z + h / 2), "Paper aged", rot=(0, 0, a))
        z += h + 0.002


def _stamp(p: Piece, at, handle: str = "Wood dark") -> None:
    """A rubber stamp: a turned handle on a brass block, inked face down."""
    x, y, z = at
    p.box((0.068, 0.048, 0.012), (x, y, z + 0.006), "Ink")
    p.box((0.075, 0.055, 0.026), (x, y, z + 0.025), "Brass")
    p.lathe([(0.016, z + 0.038), (0.011, z + 0.05), (0.01, z + 0.07), (0.022, z + 0.088), (0.024, z + 0.1),
             (0.016, z + 0.114), (0, z + 0.118)], (x, y, 0), handle, segments=12)


def _bankers_lamp(p: Piece, at, shade: str) -> None:
    """A banker's lamp: a stepped brass foot, a stem bent forward, a cased
    half-round shade (green glass outside, warm light under it) with brass
    end caps, and a pull chain."""
    x, y, z = at
    p.lathe([(0.075, z), (0.075, z + 0.012), (0.062, z + 0.022), (0.026, z + 0.03), (0.016, z + 0.04), (0, z + 0.04)],
            (x, y, 0), "Brass", segments=20)
    p.tube([(x, y, z + 0.035), (x, y, z + 0.27), (x, y - 0.04, z + 0.31), (x, y - 0.08, z + 0.32)], 0.008, "Brass", segments=8)
    sy, sz = y - 0.1, z + 0.3
    # The shade: a half-round shell, its section drawn across Y and run
    # along X.
    n = 10
    outer = [(math.cos(math.pi * k / n) * 0.075, math.sin(math.pi * k / n) * 0.06) for k in range(n + 1)]
    inner = [(c * 0.85, s * 0.8) for c, s in reversed(outer)]
    p.prism(outer + inner, 0.3, (x, sy, sz), shade, rot=(0, 0, 90))
    p.box((0.29, 0.11, 0.006), (x, sy, sz + 0.004), "Sodium lamp")
    p.light((x, sy, sz - 0.1), intensity=0.7, reach=2)
    for e in (-1, 1):
        p.prism([(math.cos(math.pi * k / n) * 0.08, math.sin(math.pi * k / n) * 0.065) for k in range(n + 1)],
                0.012, (x + e * 0.156, sy, sz), "Brass", rot=(0, 0, 90))
    p.cable((x + 0.07, sy - 0.05, sz), (x + 0.07, sy - 0.06, sz - 0.1), 0.002, "Brass", steps=2)
    p.sphere(0.008, (x + 0.07, sy - 0.06, sz - 0.105), "Brass", segments=8, rings=5)


def _raised_panel(p: Piece, x0, x1, z0, z1, y, mat, frame, side=-1, depth=0.02) -> None:
    """A raised panel on a face at y (facing side): a moulded frame proud of
    it and the field inside proud of the frame's inner edge, each in its own
    plane."""
    f = 0.035
    def proud(a, b):
        return sorted((y + side * a, y + side * b))
    ya, yb = proud(0.0, depth)
    for xa, xb, za, zb in ((x0, x1, z0, z0 + f), (x0, x1, z1 - f, z1), (x0, x0 + f, z0 + f, z1 - f), (x1 - f, x1, z0 + f, z1 - f)):
        p.span((xa, ya, za), (xb, yb, zb), frame)
    fa, fb = proud(0.0, depth * 0.6)
    p.span((x0 + f, fa, z0 + f), (x1 - f, fb, z1 - f), mat)


def _medallion(p: Piece, at, radius, facing: str = "-Y") -> None:
    """A Registrar's seal: a turned brass disc with a raised rim, a navy
    field, and an open book in brass at its middle. Its back is at at, on a
    face looking along facing ("-Y" on a wall or panel, "+Z" on a table)."""
    rot = (90, 0, 0) if facing == "-Y" else (0, 0, 0)
    d = Vector((0, -1, 0)) if facing == "-Y" else Vector((0, 0, 1))
    a = Vector(at)
    R = radius
    # Layered outwards with a clear gap between each (none share a plane).
    p.lathe([(R, 0), (R, 0.02), (R * 0.86, 0.022), (R * 0.8, 0.006), (0, 0.006)], a, "Brass", rot=rot, segments=16)
    p.cyl(R * 0.78, 0.0115, a + d * 0.01025, "Repaint navy", rot=rot, segments=14)
    for s in (-1, 1):
        page = [(0, -R * 0.3), (s * R * 0.42, -R * 0.25), (s * R * 0.45, R * 0.3), (0, R * 0.25)]
        p.prism(page if s > 0 else list(reversed(page)), 0.01, a + d * 0.021, "Brass", rot=(0, 0, 0) if facing == "-Y" else (90, 0, 0))


# The counter -------------------------------------------------------------------

def registrar_counter(style: Style) -> Piece:
    """The Registrars' counter, where contracts are filed and stamped: 4 m of
    panelled ship steel under a wooden top, a seal on every panel, a brass
    kick rail, and two brass grille windows with arched heads. Between the
    windows it's 0.95 m high: vaultable, for anyone in a hurry to get at the
    records."""
    p = Piece("registrar_counter", "prop", "4 m counter, 0.95 m high: vaultable between its two grille windows", budget=40000)
    r = rng(style, p.name)
    w, d, h = 2.0, 0.35, 0.95
    # The carcass, the plinth, and the top with its moulded front edge.
    p.span((-w + 0.02, -d + 0.05, 0.12), (w - 0.02, d, h - 0.07), "Hull alloy")
    p.span((-w + 0.03, -d + 0.04, 0), (w - 0.03, d - 0.01, 0.12), "Hull dark")
    p.prism([(-d, 0), (d, 0), (d, 0.045), (-d + 0.03, 0.045), (-d + 0.01, 0.035), (-d, 0.02)],
            2 * w, (0, 0, h - 0.07), "Wood dark", rot=(0, 0, 90))
    p.span((-w + 0.01, -d + 0.01, h - 0.025), (w - 0.01, d - 0.005, h), "Wood")
    p.collider((2 * w, 2 * d, h), (0, 0, h / 2))
    # The public face: four raised navy panels, each with its seal.
    for i in range(4):
        x = -w + 0.5 + i * 1.0
        _raised_panel(p, x - 0.435, x + 0.435, 0.16, h - 0.11, -d + 0.05, "Charter navy", "Repaint navy")
        _medallion(p, (x, -d + 0.03, 0.5), 0.1)
    # Pilasters between the panels, and the kick rail on brass stand-offs.
    for i in range(5):
        x = -w + 0.03 + i * (2 * w - 0.06) / 4
        p.span((x - 0.03, -d + 0.02, 0.12), (x + 0.03, -d + 0.05, h - 0.07), "Hull dark")
    p.tube([(-w + 0.1, -d - 0.035, 0.07), (w - 0.1, -d - 0.035, 0.07)], 0.018, "Brass", segments=10)
    for x in (-1.6, -0.6, 0.6, 1.6):
        p.cyl(0.01, 0.05, (x, -d - 0.01, 0.07), "Brass", rot=(90, 0, 0), segments=8)
    # The windows: posts, an arched head, brass bars, a pass-through slot
    # and a numbered plate.
    for n, x in enumerate((-1.3, 1.3), start=1):
        ww, top = 0.45, h + 0.75
        for sx in (-1, 1):
            p.span((x + sx * ww - 0.045, -0.045, h), (x + sx * ww + 0.045, 0.045, top), "Hull dark")
            p.lathe([(0.035, top), (0.04, top + 0.02), (0.02, top + 0.05), (0, top + 0.07)], (x + sx * ww, 0, 0), "Brass", segments=12)
        p.torus(ww, 0.03, (x, 0, top - 0.02), "Hull dark", rot=(90, 0, 0), segments=24, sides=6, arc=180)
        p.span((x - ww, -0.03, h + 0.16), (x + ww, 0.03, h + 0.2), "Brass")
        for i in range(9):
            bx = x - ww + 0.05 + i * (2 * ww - 0.1) / 8
            bz = top - 0.02 + math.sqrt(max(0.0, ww * ww - (bx - x) ** 2)) - 0.03
            p.cyl(0.007, bz - (h + 0.2), (bx, 0, (h + 0.2 + bz) / 2), "Brass", segments=6)
        p.span((x - 0.2, -0.055, top + ww - 0.08), (x + 0.2, -0.03, top + ww + 0.06), "Brass")
        p.stencil(str(n), (x, -0.06, top + ww - 0.01), 0.09, "Repaint navy")
        p.collider((2 * ww + 0.09, 0.09, top - h), (x, 0, (h + top) / 2))
        # Paper at the window, a desk bell beside it.
        _papers(p, r, (x - 0.15, -0.18, h), 3)
        p.lathe([(0.055, h), (0.055, h + 0.008), (0.05, h + 0.012), (0.045, h + 0.035), (0.025, h + 0.055), (0, h + 0.06)],
                (x + 0.28, -0.2, 0), "Brass", segments=16)
        p.cyl(0.008, 0.015, (x + 0.28, -0.2, h + 0.067), "Brass", segments=8)
    # On the clerks' side: stamps, ledgers and in-trays.
    _ledgers(p, r, (0.3, 0.17, h), 3)
    _stamp(p, (-0.35, 0.18, h))
    _stamp(p, (-0.5, 0.15, h), handle="Fabric red")
    for x in (-0.1, 0.65):
        p.span((x - 0.17, 0.05, h), (x + 0.17, 0.3, h + 0.006), "Hull alloy")
        for s in (-1, 1):
            p.span((x + s * 0.17 - 0.006, 0.05, h + 0.006), (x + s * 0.17 + 0.006, 0.3, h + 0.07), "Hull alloy")
        _papers(p, r, (x, 0.175, h + 0.006), 4)
    return p


def registrar_desk(style: Style) -> Piece:
    """A Registrar's desk, for arbitrations and the slower filings: a steel
    office desk (Poly Haven's metal_office_desk), its drawers to the
    Registrar on the +Y side, a green leather blotter, stamps, ledgers, a
    clipboard and a banker's lamp."""
    p = Sourced("registrar_desk", "prop", "1.6 × 0.8 m desk, 0.76 m high", asset="metal_office_desk", res="1k",
                height=0.76, stretch=(0.83, 0.87, 1.0), turns=2, budget=24000)
    r = rng(style, p.name)
    top = 0.76
    p.collider((1.6, 0.8, top), (0, 0, top / 2))
    p.span((-0.5, -0.24, top), (0.5, 0.24, top + 0.008), "Leather green")
    p.span((-0.5, -0.24, top + 0.008), (-0.47, 0.24, top + 0.016), "Leather")
    p.span((0.47, -0.24, top + 0.008), (0.5, 0.24, top + 0.016), "Leather")
    _ledgers(p, r, (0.6, 0.05, top), 4)
    _papers(p, r, (0.05, -0.04, top + 0.008), 5)
    _stamp(p, (0.25, 0.2, top + 0.008))
    _stamp(p, (0.33, 0.2, top + 0.008), handle="Fabric red")
    p.box((0.12, 0.08, 0.015), (0.42, 0.2, top + 0.016), "Ink")
    _bankers_lamp(p, (-0.6, 0.22, top), "Leather green")
    p.source("clipboard", at=(-0.3, -0.05, top + 0.008), rot=(0, 0, 12), length=0.23)
    p.source("stationery_supplies", at=(-0.15, 0.25, top + 0.008), length=0.3)
    return p


def filing_cabinets(style: Style) -> Piece:
    """A 2 m wall of filing drawers: every contract the Exchange has filed is
    in drawers like these. Steel carcass, navy fronts in recessed frames, a
    brass card holder and cup pull on each, one or two replaced in another
    colour, and one drawer left open, crammed with folders."""
    p = Piece("filing_cabinets", "prop", "2 × 0.6 × 2 m of drawers", budget=40000)
    r = rng(style, p.name)
    w, d, h = 1.0, 0.3, 2.0
    p.span((-w, -d + 0.03, 0.1), (w, d, h - 0.06), "Hull alloy")
    p.span((-w + 0.02, -d + 0.05, 0), (w - 0.02, d - 0.02, 0.1), "Hull dark")
    p.prism([(-d - 0.02, 0), (d, 0), (d, 0.06), (-d - 0.02, 0.06), (-d - 0.03, 0.045), (-d - 0.03, 0.015)],
            2 * w, (0, 0, h - 0.06), "Hull dark", rot=(0, 0, 90))
    p.collider((2 * w, 2 * d, h), (0, 0, h / 2))
    cols, rows = 4, 6
    cw, rh = 2 * w / cols, (h - 0.18) / rows
    # The carcass's dividers, proud of the fronts.
    for c in range(cols + 1):
        x = -w + c * cw
        p.span((max(-w, x - 0.012), -d, 0.1), (min(w, x + 0.012), -d + 0.03, h - 0.06), "Hull alloy")
    for k in range(rows + 1):
        z = 0.1 + k * rh
        p.span((-w, -d - 0.004, z - 0.01), (w, -d + 0.03, z + 0.01), "Hull alloy")
    for c in range(cols):
        for row in range(rows):
            if c == 2 and row == 2:
                continue
            x0, x1 = -w + c * cw + 0.016, -w + (c + 1) * cw - 0.016
            z0, z1 = 0.1 + row * rh + 0.014, 0.1 + (row + 1) * rh - 0.014
            front = "Repaint navy" if r.random() > 0.12 else r.choice(REPAINTS)
            p.span((x0, -d + 0.008, z0), (x1, -d + 0.03, z1), front)
            mx = (x0 + x1) / 2
            # The card holder (a brass frame round a paper card) and the pull.
            p.span((mx - 0.075, -d - 0.004, z1 - 0.1), (mx + 0.075, -d + 0.008, z1 - 0.045), "Brass")
            p.span((mx - 0.062, -d - 0.008, z1 - 0.092), (mx + 0.062, -d - 0.004, z1 - 0.053), "Paper" if r.random() > 0.25 else "Paper aged")
            with p.painted():
                p.span((mx - 0.045, -d - 0.013, z1 - 0.076), (mx + 0.03, -d - 0.008, z1 - 0.07), "Ink")
            p.torus(0.04, 0.009, (mx, -d - 0.005, z0 + 0.075), "Brass", rot=(90, 0, 0), segments=6, sides=6, arc=180)
    # The open drawer and its folders.
    x0, x1 = -w + 2 * cw + 0.016, -w + 3 * cw - 0.016
    z0, z1 = 0.1 + 2 * rh + 0.014, 0.1 + 3 * rh - 0.014
    p.span((x0, -d - 0.36, z0), (x1, -d - 0.34, z1), "Repaint navy")
    for s in (x0, x1 - 0.012):
        p.span((s, -d - 0.34, z0), (s + 0.012, -d + 0.02, z1 - 0.06), "Hull alloy")
    p.span((x0, -d - 0.34, z0), (x1, -d + 0.02, z0 + 0.012), "Hull alloy")
    for i in range(7):
        y = -d - 0.3 + i * 0.045
        hh = rh - 0.06 + r.uniform(-0.02, 0.04)
        p.box((cw - 0.08, 0.008, hh), ((x0 + x1) / 2, y, z0 + 0.012 + hh / 2), r.choice(["Paper aged", "Fabric sand", "Repaint cream"]), rot=(r.uniform(-5, 5), 0, 0))
        p.box((0.06, 0.009, 0.03), ((x0 + x1) / 2 - 0.08 + (i % 3) * 0.07, y, z0 + 0.012 + hh + 0.012), r.choice(["Fabric red", "Brass", "Repaint teal"]))
    return p


def terminal_row(style: Style) -> Piece:
    """A bench of four public terminals, where contracts are posted and
    browsed: a long console of ship panelling, a hooded screen and a key
    tray at each place, cables looped behind, numbered stations and a
    swivel stool at each."""
    p = Piece("terminal_row", "prop", "3.2 m console of four terminals, desk 0.82 m high", budget=30000)
    w, d, h = 1.6, 0.35, 0.82
    p.span((-w + 0.02, -d + 0.1, 0.1), (w - 0.02, d, h - 0.045), "Hull dark")
    p.span((-w + 0.04, -d + 0.12, 0), (w - 0.04, d - 0.02, 0.1), "Hull dark")
    p.prism([(-d - 0.02, 0), (d + 0.02, 0), (d + 0.02, 0.045), (-d, 0.045), (-d - 0.02, 0.03)],
            2 * w, (0, 0, h - 0.045), "Hull alloy", rot=(0, 0, 90))
    for i in range(4):
        x = -w + 0.4 + i * 0.8
        _raised_panel(p, x - 0.37, x + 0.37, 0.14, h - 0.09, -d + 0.1, "Repaint navy", "Hull alloy")
    p.collider((2 * w + 0.04, 2 * d + 0.04, h), (0, 0, h / 2))
    p.collider((2 * w, 0.3, 0.55), (0, 0.2, h + 0.275))
    for i in range(4):
        x = -w + 0.4 + i * 0.8
        p.stencil(str(i + 1), (x, -d + 0.081, 0.28), 0.12, "Brass")
        # The screen in its housing, tilted back under a hood.
        p.box((0.62, 0.16, 0.44), (x, 0.2, h + 0.3), "Hull alloy", rot=(-12, 0, 0))
        p.box((0.54, 0.02, 0.35), (x, 0.111, h + 0.31), "Hull dark", rot=(-12, 0, 0))
        p.box((0.5, 0.02, 0.31), (x, 0.101, h + 0.31), "Status blue", rot=(-12, 0, 0))
        p.prism([(-0.34, 0), (0.34, 0), (0.32, 0.03), (-0.32, 0.03)], 0.24, (x, 0.16, h + 0.53), "Hull dark", rot=(-12, 0, 0))
        tilt = Matrix.Rotation(math.radians(-12), 4, "X")
        with p.painted():
            for j in range(4):
                # On the screen's own (tilted) face, 6 mm proud of it.
                at = Vector((x - 0.04 * (j % 2), 0.101, h + 0.31)) + (tilt @ Vector((0, -0.016, -0.09 + j * 0.06)))
                p.box((0.36 - 0.1 * (j % 2), 0.004, 0.022), at, "Screen", rot=(-12, 0, 0))
        # The key tray: a sloped block, keys in rows.
        p.box((0.46, 0.2, 0.035), (x, -0.13, h + 0.017), "Hull dark", rot=(5, 0, 0))
        for kr in range(3):
            # A row of keys: one block, the keys' gaps painted on it.
            ky, kz = -0.19 + kr * 0.045, h + 0.035 + kr * 0.004
            p.box((0.4, 0.034, 0.014), (x, ky, kz), "Hull alloy", rot=(5, 0, 0))
            with p.painted():
                for kc in range(1, 10):
                    p.box((0.004, 0.034, 0.003), (x - 0.2 + kc * 0.04, ky, kz + 0.0125), "Hull dark", rot=(5, 0, 0))
        # A divider between places, and the cable looping off the back.
        if i < 3:
            p.span((x + 0.385, 0.02, h), (x + 0.415, d, h + 0.48), "Repaint navy")
        p.cable((x + 0.2, 0.28, h + 0.12), (x + 0.3, 0.33, 0.12), 0.012, "Rubber", sag=0.05, steps=6)
        # The stool: a cast foot, a column with a footring, a padded seat.
        p.lathe([(0.2, 0), (0.2, 0.015), (0.06, 0.04), (0.035, 0.06), (0, 0.06)], (x, -0.72, 0), "Hull dark", segments=16)
        p.cyl(0.028, 0.56, (x, -0.72, 0.34), "Steel", segments=10)
        p.torus(0.16, 0.012, (x, -0.72, 0.28), "Steel", segments=16, sides=6)
        p.lathe([(0.17, 0.6), (0.18, 0.62), (0.18, 0.66), (0.16, 0.68), (0, 0.685)], (x, -0.72, 0), "Leather", segments=16)
    return p


def queue_rail(style: Style) -> Piece:
    """Two brass stanchions and a red rope: where the queue for the counter
    starts. Exactly 2 m, so a run of them meets; 0.9 m high, so the
    impatient vault it."""
    p = Piece("queue_rail", "prop", "2 m of rope between posts, 0.9 m high: vaultable")
    for x in (-0.86, 0.86):
        p.lathe([(0.14, 0), (0.14, 0.012), (0.12, 0.03), (0.05, 0.06), (0.03, 0.08), (0, 0.08)], (x, 0, 0), "Hull dark", segments=20)
        p.lathe([(0.026, 0.08), (0.022, 0.4), (0.026, 0.42), (0.022, 0.44), (0.02, 0.84), (0, 0.84)], (x, 0, 0), "Brass", segments=12)
        p.lathe([(0.03, 0.84), (0.04, 0.85), (0.045, 0.87), (0.035, 0.9), (0, 0.905)], (x, 0, 0), "Brass", segments=12)
        p.torus(0.03, 0.007, (x - math.copysign(0.04, x), 0, 0.8), "Brass", rot=(90, 0, 0), segments=10, sides=6)
    p.cable((-0.82, 0, 0.8), (0.82, 0, 0.8), 0.018, "Fabric red", sag=0.1, segments=10, steps=14)
    for x in (-0.8, 0.8):
        p.cyl(0.022, 0.05, (x, 0, 0.8), "Brass", rot=(0, 90, 0), segments=10)
    p.collider((2.0, 0.16, 0.9), (0, 0, 0.45))
    return p


def floor_bell(style: Style) -> Piece:
    """The Exchange's floor bell, rung when a big contract is filed: the
    Second Light's ship's bell, salvaged, hung from a riveted beam between
    two hull ribs on a stepped plinth with the Accord's year, 41, in brass."""
    p = Piece("floor_bell", "prop", "1.6 × 0.8 m plinth, bell frame 2.7 m high", budget=30000)
    # The plinth: two steps of dark steel, a brass edge, the year's plate.
    p.solid((-0.8, -0.4, 0), (0.8, 0.4, 0.12), "Hull dark")
    p.span((-0.75, -0.35, 0.14), (0.75, 0.35, 0.2), "Hull dark")
    p.collider((1.5, 0.7, 0.08), (0, 0, 0.16))
    p.prism([(-0.41, 0), (0.41, 0), (0.41, 0.02), (-0.41, 0.02)], 1.64, (0, 0, 0.12), "Brass", rot=(0, 0, 90))
    p.span((-0.22, -0.36, 0.145), (0.22, -0.35, 0.195), "Brass")
    p.stencil("41", (0, -0.367, 0.17), 0.04, "Repaint navy")
    top = 2.7
    # The ribs: I-beams with flanges, riveted, and knees to the beam.
    for x in (-0.68, 0.68):
        for s in (-1, 1):
            p.span((x - 0.075, s * 0.065 - 0.012, 0.2), (x + 0.075, s * 0.065 + 0.012, top - 0.1), "Hull alloy")
        p.span((x - 0.012, -0.065, 0.2), (x + 0.012, 0.065, top - 0.1), "Hull alloy")
        p.collider((0.15, 0.15, top - 0.3), (x, 0, 0.2 + (top - 0.3) / 2))
        for z in [0.4 + k * 0.25 for k in range(9)]:
            for dx in (-0.05, 0.05):
                p.cyl(0.008, 0.006, (x + dx, -0.079, z), "Hull dark", rot=(90, 0, 0), segments=6)
        p.box((0.05, 0.1, 0.5), (x * 0.8, 0, top - 0.42), "Hull alloy", rot=(0, math.copysign(42, -x), 0))
        p.span((x - 0.1, -0.09, top - 0.1), (x + 0.1, 0.09, top), "Brass")
    # The beam, its straps and bolts.
    p.span((-0.76, -0.09, top - 0.3), (0.76, 0.09, top - 0.12), "Hull dark")
    for x in (-0.2, 0.2):
        p.span((x - 0.02, -0.1, top - 0.42), (x + 0.02, 0.1, top - 0.1), "Hull alloy")
        p.cyl(0.015, 0.23, (x, 0, top - 0.37), "Steel", rot=(90, 0, 0), segments=8)
    # The bell: crown, shoulder, waist, sound bow and lip; its clapper.
    mouth, crown = top - 1.25, top - 0.42
    p.lathe([(0, mouth + 0.06), (0.36, mouth + 0.04), (0.41, mouth), (0.42, mouth + 0.03), (0.4, mouth + 0.1),
             (0.33, mouth + 0.3), (0.29, mouth + 0.5), (0.27, crown - 0.16), (0.24, crown - 0.07), (0.17, crown - 0.02),
             (0.05, crown), (0, crown)], (0, 0, 0), "Brass", segments=32)
    for z, rr in ((mouth + 0.06, 0.395), (mouth + 0.42, 0.3), (crown - 0.12, 0.262)):
        p.torus(rr, 0.01, (0, 0, z), "Brass", segments=32, sides=6)
    p.span((-0.12, -0.05, crown - 0.01), (0.12, 0.05, top - 0.3), "Hull alloy")
    p.torus(0.07, 0.02, (0, 0, crown + 0.02), "Brass", rot=(90, 0, 0), segments=12, sides=6)
    p.cyl(0.015, 0.6, (0, 0, mouth + 0.32), "Hull dark", segments=8)
    p.sphere(0.075, (0, 0, mouth + 0.0), "Hull dark", segments=14, rings=8)
    # The pull: a rope from the clapper to a knotted end.
    p.cable((0, 0, mouth - 0.06), (0.06, -0.06, 0.95), 0.012, "Fabric red", sag=0.02, steps=8)
    p.lathe([(0.025, 0.82), (0.045, 0.86), (0.04, 0.9), (0.015, 0.95)], (0.06, -0.06, 0), "Fabric red", segments=10)
    p.collider((0.84, 0.84, crown - mouth), (0, 0, (mouth + crown) / 2))
    return p


def archive_shelves(style: Style) -> Piece:
    """Steel shelving of the records, exactly 2 m wide: perforated uprights,
    cross-braced at the back, five shelves of boxed filings back to 41 AL,
    labelled, with the odd row of ledgers on their spines."""
    p = Piece("archive_shelves", "prop", "2 × 0.5 × 2.6 m shelving", budget=36000)
    r = rng(style, p.name)
    w, d, h = 1.0, 0.25, 2.6
    for x in (-w + 0.03, 0.0, w - 0.03):
        for y in (-d + 0.02, d - 0.02):
            # Angle uprights: a flange across the front, one along the side,
            # perforated.
            p.span((x - 0.03, y - 0.032, 0), (x + 0.03, y - 0.026, h), "Hull dark")
            p.span((x - 0.003, y - 0.026, 0), (x + 0.003, y + 0.02, h), "Hull dark")
            with p.painted():
                for k in range(26):
                    p.box((0.012, 0.004, 0.025), (x, y - 0.039, 0.1 + k * 0.1), "Ink")
    # Cross-bracing on the back.
    import math as _m
    a = _m.degrees(_m.atan2(h - 0.2, w - 0.06))
    for cx in (-w / 2, w / 2):
        for s in (-1, 1):
            p.box((_m.hypot(w - 0.06, h - 0.2), 0.006, 0.025), (cx, d - 0.012, h / 2), "Hull alloy", rot=(0, s * a, 0))
    shelves = [0.08, 0.6, 1.12, 1.64, 2.16]
    for z in shelves + [h - 0.02]:
        # Each shelf hung between the uprights, not through them.
        for x0, x1 in ((-w + 0.06, -0.03), (0.03, w - 0.06)):
            p.span((x0, -d, z - 0.02), (x1, d - 0.006, z), "Hull alloy")
            p.span((x0, -d - 0.006, z - 0.045), (x1, -d, z), "Hull alloy")
    p.collider((2 * w, 2 * d, h), (0, 0, h / 2))
    for si, z in enumerate(shelves):
        for x0, x1 in ((-w + 0.07, -0.04), (0.04, w - 0.07)):
            x = x0
            if si == 3 and x0 < 0:
                # Ledgers on their spines, banded.
                while x < x1 - 0.07:
                    t = r.uniform(0.045, 0.07)
                    hh = r.uniform(0.3, 0.4)
                    p.span((x, -d + 0.05, z), (x + t, d - 0.07, z + hh), r.choice(BINDINGS))
                    p.span((x + 0.006, -d + 0.07, z + 0.01), (x + t - 0.006, d - 0.06, z + hh - 0.012), "Paper aged")
                    for band in (0.2, 0.75):
                        p.span((x - 0.005, -d + 0.044, z + hh * band), (x + t + 0.005, -d + 0.07, z + hh * band + 0.02), "Brass")
                    x += t + 0.004
                continue
            while x < x1 - 0.2:
                bw = r.uniform(0.25, 0.34)
                if x + bw > x1:
                    break
                if r.random() < 0.1:
                    x += bw + 0.02
                    continue
                bh = r.uniform(0.27, 0.36)
                box = r.choice(["Paper aged", "Paper aged", "Repaint cream", "Wood"])
                # A lidded archive box: the body, the lid's lip, a hand hole,
                # a label and the year in ink.
                p.span((x, -d + 0.04, z), (x + bw, d - 0.06, z + bh - 0.03), box)
                p.span((x - 0.004, -d + 0.036, z + bh - 0.035), (x + bw + 0.004, d - 0.056, z + bh), box)
                p.span((x + bw * 0.38, -d + 0.034, z + bh * 0.62), (x + bw * 0.62, -d + 0.04, z + bh * 0.72), "Hull dark")
                p.span((x + bw * 0.2, -d + 0.035, z + bh * 0.2), (x + bw * 0.8, -d + 0.04, z + bh * 0.5), "Paper")
                p.stencil(str(r.randint(41, 87)), (x + bw / 2, -d + 0.027, z + bh * 0.35), 0.06, "Ink")
                x += bw + 0.012
    return p


def registrar_seal_emblem(style: Style) -> Piece:
    """The Registrars' seal, 1.25 m across, for the wall behind the counter:
    a notched brass ring, a navy field lettered with the Accord's year, and
    an open book, the record, embossed in brass at its heart. Its back goes
    on a wall's face, its origin at its middle."""
    p = Piece("registrar_seal_emblem", "prop", "1.25 m brass wall seal; no collider")
    # Turned in rings from the wall out: a dark backing, the brass rim with
    # its mouldings, the navy field, the inner brass boss.
    p.lathe([(0.62, 0), (0.62, 0.03), (0.6, 0.045), (0.0, 0.045)], (0, 0, 0), "Hull dark", rot=(90, 0, 0), segments=32)
    p.lathe([(0.6, 0.045), (0.6, 0.07), (0.585, 0.085), (0.56, 0.09), (0.52, 0.08), (0.5, 0.06), (0.5, 0.05)],
            (0, 0, 0), "Brass", rot=(90, 0, 0), segments=32)
    p.cyl(0.5, 0.025, (0, -0.0625, 0), "Repaint navy", rot=(90, 0, 0), segments=32)
    for i in range(32):
        a = math.tau * i / 32
        p.box((0.035, 0.02, 0.06), (math.cos(a) * 0.455, -0.08, math.sin(a) * 0.455), "Brass", rot=(0, -math.degrees(a) + 90, 0))
    p.lathe([(0.34, 0.075), (0.34, 0.09), (0.32, 0.1), (0.3, 0.095), (0, 0.095)], (0, 0, 0), "Brass", rot=(90, 0, 0), segments=28)
    p.cyl(0.29, 0.012, (0, -0.1, 0), "Repaint navy", rot=(90, 0, 0), segments=24)
    # The open book: two pages bowed up from the spine, ruled lines.
    for s in (-1, 1):
        page = [(0, -0.12), (s * 0.1, -0.13), (s * 0.2, -0.11), (s * 0.21, 0.14), (s * 0.1, 0.12), (0, 0.13)]
        p.prism(page if s > 0 else list(reversed(page)), 0.025, (s * 0.008, -0.115, -0.01), "Brass")
        with p.painted():
            for k in range(4):
                p.box((0.13, 0.004, 0.012), (s * 0.115, -0.134, 0.07 - k * 0.05), "Repaint navy", rot=(0, -s * 4, 0))
    p.cyl(0.012, 0.03, (0, -0.115, 0.0), "Brass", rot=(0, 0, 0), segments=8)
    p.stencil("41", (0, -0.082, -0.4), 0.075, "Brass")
    p.stencil("41", (0, -0.082, 0.4), 0.075, "Brass")
    return p


def waiting_bench(style: Style) -> Piece:
    """A bench for the waiting: a painted wooden settle (Poly Haven's
    painted_wooden_bench) drawn out to 1.8 m, its back worn pale where
    people lean."""
    p = Sourced("waiting_bench", "prop", "1.8 m bench, seat 0.46 m high", asset="painted_wooden_bench", res="1k",
                height=0.9, stretch=(1.55, 1.0, 1.0))
    p.collider((1.8, 0.5, 0.48), (0, 0.0, 0.24))
    p.collider((1.8, 0.1, 0.42), (0, 0.2, 0.69))
    return p


def pendant_lamp(style: Style) -> Piece:
    """A pendant lamp hung from the cargo bay's deckhead at 3.6 m: a ceiling
    rose, and an industrial lamp (Poly Haven's hanging_industrial_lamp) on
    its chain down to 2.5 m."""
    p = Sourced("pendant_lamp", "prop", "lamp hung from 3.6 m to 2.5 m; no collider", asset="hanging_industrial_lamp",
                res="1k", height=1.08, at=(0, 0, 2.5))
    p.lathe([(0.12, DECK), (0.12, DECK - 0.02), (0.1, DECK - 0.035), (0.04, DECK - 0.05), (0, DECK - 0.05)], (0, 0, 0), "Hull dark", segments=20)
    p.light((0, 0, 2.35), intensity=1.4, reach=8)
    return p


def stamp_station(style: Style) -> Piece:
    """A standing desk where filings get their stamps: a panelled pedestal,
    a sloped writing top with a ledge, a rack of stamps, ink pads, a spike of
    finished papers and a pot of pens (Poly Haven's stationery)."""
    p = Piece("stamp_station", "prop", "1 × 0.6 m standing desk, 1.05 m high")
    r = rng(style, p.name)
    p.span((-0.47, -0.24, 0), (0.47, 0.3, 0.07), "Hull dark")
    p.span((-0.42, -0.19, 0.07), (0.42, 0.26, 0.93), "Hull alloy")
    _raised_panel(p, -0.38, 0.38, 0.14, 0.86, -0.19, "Repaint navy", "Hull dark")
    _medallion(p, (0, -0.21, 0.55), 0.08)
    p.box((1.0, 0.5, 0.04), (0, -0.06, 0.985), "Wood dark", rot=(10, 0, 0))
    p.box((1.0, 0.03, 0.035), (0, -0.31, 0.95), "Wood dark", rot=(10, 0, 0))
    p.span((-0.5, 0.18, 0.95), (0.5, 0.3, 1.05), "Wood dark")
    p.collider((1.0, 0.6, 1.05), (0, 0.0, 0.525))
    # The rack of stamps along the back.
    p.span((-0.46, 0.2, 1.05), (0.46, 0.29, 1.06), "Brass")
    for i in range(6):
        _stamp(p, (-0.38 + i * 0.12, 0.245, 1.06), handle=r.choice(["Wood dark", "Fabric red", "Hull dark"]))
    # On the slope: ink pads in tins, a filing half stamped.
    for x, ink in ((-0.32, "Ink"), (-0.17, "Wax red")):
        p.box((0.12, 0.08, 0.02), (x, -0.12, 1.0), "Hull dark", rot=(10, 0, 0))
        p.box((0.1, 0.06, 0.006), (x, -0.122, 1.012), ink, rot=(10, 0, 0))
    p.box((0.21, 0.3, 0.004), (0.12, -0.07, 1.012), "Paper", rot=(10, 0, 6))
    with p.painted():
        p.box((0.05, 0.05, 0.003), (0.17, -0.12, 1.017), "Wax red", rot=(10, 0, 6))
        for k in range(5):
            p.box((0.14, 0.006, 0.002), (0.1, -0.02 - k * 0.03, 1.024 - k * 0.005), "Ink", rot=(10, 0, 6))
    # The spike, and a pot of pens.
    p.cyl(0.05, 0.02, (0.4, 0.245, 1.07), "Brass", segments=16)
    p.cyl(0.003, 0.2, (0.4, 0.245, 1.18), "Steel", segments=6, radius2=0.001)
    for i in range(6):
        p.box((0.14, 0.18, 0.003), (0.4, 0.245, 1.09 + i * 0.012), r.choice(PAPERWORK), rot=(0, 0, r.uniform(-25, 25)))
    p.source("stationery_supplies", at=(0.25, 0.245, 1.06), length=0.18)
    return p


def arbitration_table(style: Style) -> Piece:
    """The table where disputes are heard: a long table under a cloth (Poly
    Haven's dining_table), a brass Registrar's seal in the middle, the case
    papers, water, three tufted chairs (dining_chair_02) on the -Y side for
    the parties and two on the +Y side for the arbiters."""
    p = Sourced("arbitration_table", "prop", "2.4 × 1 m table with five chairs", asset="dining_table", res="1k",
                length=2.4, stretch=(1.0, 0.72, 0.86), budget=60000)
    r = rng(style, p.name)
    top = 0.76
    p.collider((2.4, 1.0, top), (0, 0, top / 2))
    _medallion(p, (0, 0, top + 0.004), 0.12, facing="+Z")
    for x in (-0.7, 0, 0.7):
        _papers(p, r, (x, -0.28, top + 0.004), 4)
        p.source("dining_chair_02", at=(x, -0.82, 0), rot=(0, 0, 180), height=0.97)
        p.collider((0.48, 0.5, 0.95), (x, -0.82, 0.475))
    for x in (-0.45, 0.45):
        _ledgers(p, r, (x, 0.26, top + 0.004), 2)
        p.source("dining_chair_02", at=(x, 0.82, 0), height=0.97)
        p.collider((0.48, 0.5, 0.95), (x, 0.82, 0.475))
    # A carafe and two glasses.
    p.lathe([(0, top), (0.055, top), (0.07, top + 0.06), (0.06, top + 0.14), (0.028, top + 0.2), (0.03, top + 0.25),
             (0.022, top + 0.25), (0, top + 0.24)], (0.2, 0.05, 0), "Glass", segments=16)
    for x, y in ((0.32, -0.02), (-0.25, 0.08)):
        p.lathe([(0, top), (0.028, top), (0.032, top + 0.09), (0.028, top + 0.09), (0, top + 0.005)], (x, y, 0), "Glass", segments=14)
    return p


PIECES = [registrar_counter, registrar_desk, filing_cabinets, terminal_row, queue_rail, floor_bell, archive_shelves,
          registrar_seal_emblem, waiting_bench, pendant_lamp, stamp_station, arbitration_table]
