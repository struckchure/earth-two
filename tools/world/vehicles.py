"""Vehicles: what's earned and what arrives. Players start on foot; haulers
and bikes are bought or given as gifts (docs/shared-world.md). Drifters
are the independent freighters that are the only link to Earth, and every
player steps off one, the Patience, at the Pads (docs/story.md). In Act 3
the Corvane Receivership comes down in a lander that's clean, new and
pressed, and looks wrong here on purpose (docs/look-and-feel.md).

Everything points its nose along -Y, the front, like every other piece
(the game's +Z, the way the characters face), with its wheels or feet on
z = 0. Most are Sketchfab models fitted that way and retrofitted (see
Sourced vehicles below); the trike is modelled, along +X, which is easier
to think in, and turned at the end.

Parts of different colours never share a face plane (see GUIDE.md): what's
laid on a surface stands at least 6 mm off it, and anything thinner is
paint."""
import math

from mathutils import Vector

from kit import PALETTE, REPAINTS, Piece, Style, rng, rounded, section
from polyhaven import Sourced, wheel_of

CATEGORY = "Vehicles"

PALETTE.setdefault("Tyre", (0.025, 0.022, 0.02))
PALETTE.setdefault("Hull pale", (0.42, 0.43, 0.42))
PALETTE.setdefault("Polymer", (0.06, 0.06, 0.05))
# Tarps and nets big enough to cover a load: canvas, but not in finish.py's
# cloth (which roughens cloth finely all over), so they keep to their budget;
# kit's cloth gives them their folds.
PALETTE.setdefault("Tarp", PALETTE["Canvas"])


# Wheels and running gear ---------------------------------------------------------
#
# The trike's running gear: big off-road tyres for the dust (the air's too
# thin to hover in), suspension out in the open where it can be mended,
# Crew-orange and Hazard-yellow accents. Bodies are lofted (Piece.loft),
# not cut from plate, so they crown, taper and lean in like bodywork.

ACCENT = "Hazard yellow"


def _wheel(p: Piece, x, y, z, radius, width, side: int, tread: str = "knobby", rim: str = "Gunmetal",
           cap: str = ACCENT, spokes: int = 6, both: bool = False, spoke_w: float = 0.14, disc: bool = True) -> None:
    """A wheel on an axle along Y, its outer face towards side (+1 or -1 in
    Y); both dresses both faces (a bike's). An off-road tyre with rounded
    shoulders and a real sidewall (the rim about 60% of its radius), a
    tread of staggered blocks ("knobby") or chevrons ("chevron") wrapping
    over the shoulders, and a spoked rim set back in it with a disc behind
    the spokes (or, with disc False and thin spokes, a wire wheel you see
    through), a hub and its nuts."""
    r, w = radius, width
    rr = 0.6 * r
    rot = (-90, 0, 0) if side > 0 else (90, 0, 0)
    p.lathe([(rr, -w / 2 + 0.015), (0.7 * r, -w / 2), (0.86 * r, -0.49 * w), (0.95 * r, -0.44 * w), (0.99 * r, -0.33 * w),
             (r, -0.18 * w), (r, 0.18 * w), (0.99 * r, 0.33 * w), (0.95 * r, 0.44 * w), (0.86 * r, 0.49 * w), (0.7 * r, w / 2),
             (rr, w / 2 - 0.015)], (x, y, z), "Tyre", rot=rot, segments=12, cap=False)
    blocks = max(14, round(math.tau * r / 0.13))
    lug = 0.022 * min(1.0, r / 0.45) + 0.008
    with p.plain():
        for i in range(blocks):
            a = math.tau * i / blocks
            for row in (-1, 1):
                b = a + (math.pi / blocks if row > 0 else 0)
                rot_b = (0, 90 - math.degrees(b), 0)
                c = Vector((math.cos(b), 0, math.sin(b)))
                if tread == "chevron":
                    # One bar each side, angled in towards the middle.
                    p.box((0.55 * math.tau * r / blocks, 0.42 * w, lug), (x + c.x * (r + lug / 3), y + row * 0.21 * w, z + c.z * (r + lug / 3)),
                          "Tyre", rot=(row * 28, 90 - math.degrees(b), 0))
                    continue
                # A centre block, and on every other a shoulder block that
                # wraps round onto the sidewall.
                p.box((0.6 * math.tau * r / blocks, 0.26 * w, lug), (x + c.x * (r + lug / 3), y + row * 0.14 * w, z + c.z * (r + lug / 3)),
                      "Tyre", rot=rot_b)
                if i % 2:
                    continue
                sh = 0.94 * r
                p.box((0.5 * math.tau * r / blocks, 0.16 * w, lug * 1.2),
                      (x + c.x * (sh + lug / 2), y + row * 0.43 * w, z + c.z * (sh + lug / 2)), "Tyre",
                      rot=rot_b)
    for s in ((-1, 1) if both else (side,)):
        rot_s = (-90, 0, 0) if s > 0 else (90, 0, 0)
        face = y + s * (w / 2 - 0.015)
        # The rim: its lip at the tyre's bead, the barrel stepping in to a
        # disc set well back, the spokes standing proud of the disc.
        p.lathe([(rr * 0.98, 0.0), (rr * 1.03, 0.012), (rr * 0.97, 0.03), (rr * 0.92, -0.03), (rr * 0.9, -0.06)],
                (x, face, z), rim, rot=rot_s, segments=12, cap=disc)
        hub = face - s * min(0.055, w * 0.4)
        if disc:
            with p.plain():
                p.cyl(rr * 0.9, 0.02, (x, hub, z), "Hull dark", rot=(90, 0, 0), segments=12)
        # The spokes left unbevelled: a bevel on each would multiply a
        # wheel's triangles several times.
        with p.plain():
            for i in range(spokes):
                a = math.tau * (i + 0.5) / spokes
                mid = rr * 0.55
                p.box((rr * 0.62 if disc else rr * 0.9, 0.035 if disc else 0.006, spoke_w * rr),
                      (x + math.cos(a) * mid, hub + s * (0.03 if disc else 0.0), z + math.sin(a) * mid), rim if disc else "Steel",
                      rot=(0, -math.degrees(a), 0))
        p.lathe([(0.0, 0.0), (rr * 0.3, 0.0), (rr * 0.28, 0.04), (rr * 0.18, 0.07), (0.0, 0.075)], (x, hub + s * 0.02, z), cap,
                rot=rot_s, segments=10)
        if disc:
            with p.painted():
                for i in range(spokes):
                    a = math.tau * i / spokes
                    p.cyl(0.012, 0.012, (x + math.cos(a) * rr * 0.36, hub + s * 0.056, z + math.sin(a) * rr * 0.36), "Steel",
                          rot=(90, 0, 0), segments=6)


def _wishbone(p: Piece, inner_a, inner_b, outer, radius: float = 0.03, mat: str = "Gunmetal") -> None:
    """An A-arm: two tubes from its pivots on the frame to the ball joint at
    the hub, the joint a ball."""
    p.tube([inner_a, outer, inner_b], radius, mat, segments=6)
    p.sphere(radius * 1.6, outer, "Steel", segments=6, rings=4)
    for e in (inner_a, inner_b):
        p.cyl(radius * 1.4, radius * 3, e, "Hull dark", rot=(0, 90, 0), segments=6)


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
    steps = turns * 6
    for i in range(steps + 1):
        t = i / steps
        ang = math.tau * turns * t
        pts.append(a + axis * (0.1 + 0.8 * t) + (u * math.cos(ang) + v * math.sin(ang)) * radius)
    with p.plain():
        p.tube(pts, radius * 0.18, spring, segments=3)
    for e in (a, b):
        p.sphere(radius * 0.5, e, "Hull dark", segments=5, rings=3)


# The trike, modelled -----------------------------------------------------------

TRIKE_FRONT = (0.85, -0.95)  # its axles along X, modelled with the nose at +X
TRIKE_RADII = (0.33, 0.36)
TRIKE_TRACK = 0.44


def _trike_colours(style: Style) -> tuple[str, str]:
    """The trike's paint and trim, the same for it and its wheels."""
    r = rng(style, "trike")
    return (r.choice(["Bleached", "Repaint cream", "Crew grey", "Hull alloy"]),
            r.choice(["Crew orange", "Hazard yellow", "Repaint oxide", "Repaint teal"]))


def _trike_wheel(style: Style, name: str, radius: float, width: float) -> Piece:
    """One of the trike's wheels, centred on its axle (along X), its outer
    face toward +X."""
    _, trim = _trike_colours(style)
    q = Piece(name, "wheel", f"{2 * radius:.2f} m trike wheel")
    _wheel(q, 0, 0, 0, radius, width, 1, tread="knobby", rim="Hull alloy", cap=trim, spokes=5)
    q.turn(-1)
    return q


def trike_wheel_front(style: Style) -> Piece:
    return _trike_wheel(style, "trike_wheel_front", TRIKE_RADII[0], 0.18)


def trike_wheel_rear(style: Style) -> Piece:
    return _trike_wheel(style, "trike_wheel_rear", TRIKE_RADII[1], 0.26)

def trike(style: Style) -> Piece:
    """A Fringe runner's reverse trike, after Death Stranding's: two
    wheels up front close together on wishbones under a broad faired nose
    with twin lamps, one fat tyre behind on a single-sided arm, a rider's
    saddle between, and racks fore and aft for what a porter carries,
    water cans and a crate roped down. Built to be loaded, not to be fast.
    (The reverse trike's proportions: a 1.8 m wheelbase, a front track of
    about half that, the saddle at a motorbike's height.)"""
    p = Piece("trike", "vehicle", "2.7 × 1.25 × 1.3 m reverse trike, front at -Y")
    p.budget = 30000
    paint, trim = _trike_colours(style)
    fx, rx = TRIKE_FRONT
    fr, rr_ = TRIKE_RADII
    track = TRIKE_TRACK
    # The wheels are pieces of their own (trike_wheel_front and _rear), so
    # they spin and steer; the left front first, as the game numbers them.
    for s in (1, -1):
        p.wheel((fx, s * track, fr), fr, 0.18, "trike_wheel_front", steer=True, drive=False)
    p.wheel((rx, 0, rr_), rr_, 0.26, "trike_wheel_rear", handbrake=True)
    p.seat((-0.15, 0, 0.43), exits=[(-0.3, 1.3, 0), (-0.3, -1.3, 0), (-2.3, 0, 0)], pose="ride",
           grips=[(0.24, 0.3, 1.12), (0.24, -0.3, 1.12)], pegs=[(-0.24, 0.2, 0.42), (-0.24, -0.2, 0.42)])
    p.chassis((2.7, 0.9, 0.6), (-0.05, 0, 0.75))
    # Weighted low and forward, over the front pair, as a reverse trike's
    # battery and drive are: it tips over the outside front wheel, about the
    # line from it to the back wheel, and weight near that line tips it
    # before its tyres slide.
    p.drive("trike", com=(0.4, 0, 0.25))
    # Front wheels on double wishbones from the nose's frame, a coilover
    # each, a fender hugging each tyre on a stay.
    for s in (-1, 1):
        p.box((0.08, 0.05, 0.24), (fx, s * (track - 0.13), fr), "Hull dark")
        _wishbone(p, (fx - 0.18, s * 0.16, 0.5), (fx + 0.18, s * 0.16, 0.5), (fx, s * (track - 0.13), 0.44), 0.02)
        _wishbone(p, (fx - 0.2, s * 0.16, 0.26), (fx + 0.2, s * 0.16, 0.26), (fx, s * (track - 0.13), 0.22), 0.022)
        _coilover(p, (fx - 0.04, s * (track - 0.17), 0.26), (fx - 0.1, s * 0.15, 0.7), radius=0.04, turns=7)
        p.arch((fx, s * track, fr), fr + 0.04, 0.2, trim, depth=0.014, a0=0, a1=150, lip=0.015, side=s)
        p.tube([(fx - 0.25, s * track, fr + 0.3), (fx - 0.3, s * 0.2, 0.62)], 0.014, "Hull dark", segments=6)
    # The nose: broad and faired between the front wheels, rising to the
    # bars, its lamps in a dark mask, a skid plate under it.
    nose = p.loft([(1.32, section(0.3, 0.4, 0.54, tumble=0.04, r_top=0.06, r_bottom=0.05)),
                   (1.2, section(0.46, 0.33, 0.64, crown=0.02, tumble=0.06, r_top=0.1, r_bottom=0.06)),
                   (0.85, section(0.52, 0.3, 0.78, crown=0.03, tumble=0.08, r_top=0.12, r_bottom=0.06)),
                   (0.5, section(0.48, 0.32, 0.92, crown=0.03, tumble=0.08, r_top=0.1, r_bottom=0.06)),
                   (0.3, section(0.42, 0.36, 0.98, crown=0.02, tumble=0.07, r_top=0.08, r_bottom=0.06))], paint)
    p.span((1.29, -0.12, 0.43), (1.325, 0.12, 0.52), "Hull dark")
    for y in (-0.065, 0.065):
        p.lathe([(0.0, 0.0), (0.04, 0.0), (0.045, 0.02), (0.0, 0.03)], (1.322, y, 0.475), "Hull dark", rot=(0, 90, 0), segments=12)
        p.cyl(0.032, 0.012, (1.35, y, 0.475), "Sodium lamp", rot=(0, 90, 0), segments=12)
    for s in (-1, 1):
        p.seam([nose.side(x, 0.58, s) for x in (0.35, 0.6, 0.85, 1.1)])
        # A flash of the trim colour down each flank.
        p.prism(rounded([(0.42, 0.42), (1.12, 0.44), (1.18, 0.52), (0.42, 0.56)], 0.03, 3), 0.014, (0, s * 0.247, 0), trim)
    p.seam([nose.top(0.75, y) for y in (-0.18, -0.09, 0.0, 0.09, 0.18)])
    p.prism(rounded([(0.3, 0.24), (1.15, 0.26), (1.3, 0.38), (1.2, 0.38), (0.3, 0.32)], 0.03, 3), 0.36, (0, 0, 0), "Hull dark")
    # A small rack on the nose, a case strapped to it.
    p.tube([(0.62, -0.17, 0.96), (1.1, -0.17, 0.74), (1.1, 0.17, 0.74), (0.62, 0.17, 0.96)], 0.012, "Hull dark", segments=6)
    p.loft([(0.7, section(0.3, 0.93, 1.07, r_top=0.03, r_bottom=0.02)),
            (1.0, section(0.3, 0.8, 0.94, r_top=0.03, r_bottom=0.02))], "Container green", smooth=False)
    # The middle: the battery under a tank-like cover, the saddle behind,
    # a frame spine under it all to the rear arm's pivot.
    tank = p.loft([(-0.3, section(0.34, 0.6, 0.92, crown=0.02, tumble=0.05, bulge=0.02, r_top=0.07, r_bottom=0.03)),
                   (0.0, section(0.42, 0.52, 1.0, crown=0.03, tumble=0.07, bulge=0.03, r_top=0.08, r_bottom=0.04)),
                   (0.32, section(0.4, 0.5, 1.0, crown=0.02, tumble=0.07, r_top=0.08, r_bottom=0.04))], paint)
    p.lathe([(0.0, 0.0), (0.04, 0.0), (0.04, 0.015), (0.0, 0.025)], tank.top(0.05, 0)[0], "Steel", segments=10)
    p.loft([(-0.45, section(0.36, 0.3, 0.56, r_top=0.03, r_bottom=0.04)), (0.35, section(0.36, 0.28, 0.54, r_top=0.03, r_bottom=0.04))],
           "Gunmetal")
    for s in (-1, 1):
        for x in (-0.3, -0.05, 0.2):
            p.span((x - 0.08, s * 0.186, 0.33), (x + 0.08, s * 0.192, 0.5), "Hull dark")
        p.span((-0.3, s * 0.1, 0.38), (-0.18, s * 0.28, 0.4), "Grating")
    p.loft([(-0.95, section(0.24, 0.88, 0.95, crown=0.015, r_top=0.03, r_bottom=0.01)),
            (-0.55, section(0.32, 0.88, 0.97, crown=0.02, r_top=0.04, r_bottom=0.01)),
            (-0.28, section(0.3, 0.9, 0.99, crown=0.02, r_top=0.04, r_bottom=0.01))], "Leather")
    # The tail over the rear wheel, its lamp, the big rack on it.
    p.loft([(-1.3, section(0.3, 0.74, 0.86, tumble=0.03, r_top=0.04, r_bottom=0.03)),
            (-1.0, section(0.36, 0.66, 0.88, tumble=0.03, r_top=0.04, r_bottom=0.03)),
            (-0.5, section(0.38, 0.56, 0.88, tumble=0.03, r_top=0.04, r_bottom=0.03))], paint)
    p.span((-1.305, -0.1, 0.78), (-1.29, 0.1, 0.82), "Medical red")
    p.span((-1.45, -0.42, 0.98), (-0.95, 0.42, 1.01), "Grating")
    p.tube([(-0.95, -0.44, 1.01), (-1.47, -0.44, 1.01), (-1.47, 0.44, 1.01), (-0.95, 0.44, 1.01)], 0.016, "Hull dark", segments=6)
    for y in (-0.3, 0.3):
        p.tube([(-1.0, y, 0.88), (-1.0, y, 0.99)], 0.016, "Hull dark", segments=6)
        p.tube([(-1.4, y, 0.85), (-1.4, y, 0.99)], 0.016, "Hull dark", segments=6)
    for y in (-0.24, 0.1):
        p.source("metal_jerrycan_green", at=(-1.3, y, 1.01), rot=(0, 0, 90), height=0.44)
    p.source("plastic_crate_02", at=(-1.05, 0.25, 1.01), rot=(0, 0, 0), length=0.36)
    # A strap over the load.
    p.tube([(-1.22, -0.44, 1.02), (-1.22, -0.3, 1.47), (-1.22, 0.3, 1.47), (-1.22, 0.44, 1.02)], 0.01, "Fabric olive", segments=4)
    # The rear wheel on a single-sided arm on the left, its shock.
    p.loft([(rx, section(0.05, 0.3, 0.42, y=-0.17, r_top=0.015, r_bottom=0.015)),
            (-0.4, section(0.06, 0.32, 0.5, y=-0.17, r_top=0.02, r_bottom=0.02))], trim, smooth=False)
    p.cyl(0.07, 0.36, (-0.4, 0, 0.42), "Hull dark", rot=(90, 0, 0), segments=12)
    p.cyl(0.11, 0.06, (rx, -0.14, rr_), "Hull dark", rot=(90, 0, 0), segments=14)
    _coilover(p, (-0.62, -0.13, 0.44), (-0.42, -0.1, 0.84), radius=0.04, turns=7)
    p.arch((rx, 0, rr_), rr_ + 0.05, 0.3, "Hull dark", depth=0.014, a0=50, a1=150)
    # Bars on a riser, grips, a short screen, mirrors.
    p.tube([(0.36, 0, 0.98), (0.3, 0, 1.08)], 0.025, "Hull dark", segments=6)
    p.tube([(0.24, -0.36, 1.12), (0.3, -0.18, 1.09), (0.3, 0.18, 1.09), (0.24, 0.36, 1.12)], 0.013, "Hull dark", segments=6)
    for s in (-1, 1):
        p.cyl(0.019, 0.12, (0.24, s * 0.34, 1.12), "Rubber", rot=(90, 0, 0), segments=8)
        p.tube([(0.3, s * 0.24, 1.1), (0.28, s * 0.3, 1.28)], 0.006, "Hull dark", segments=4)
        p.sphere(0.035, (0.28, s * 0.3, 1.3), "Hull dark", scale=(0.4, 1.2, 0.8), segments=8, rings=5)
    screen = rounded([(-0.13, 0.0), (0.13, 0.0), (0.15, 0.2), (-0.15, 0.2)], 0.03, 3)
    p.loft([(-0.004, screen), (0.004, screen)], "Screen", at=(0.42, 0, 0.97), rot=(0, -32, 0), smooth=False)
    p.collider((2.5, 1.0, 0.9), (-0.08, 0, 0.5))
    p.turn(-1)  # nose along -Y
    return p


# Ramps --------------------------------------------------------------------------

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


# Sourced vehicles --------------------------------------------------------------
#
# The buggy, bike, haulers, rover and both ships are built on Sketchfab
# models (sketchfab.py; CC-BY, credited in assets/world/CREDITS.txt) and
# retrofitted for the Red: repainted in the colony's colours (paint), worn
# and dusted by the finish like everything else, their guns and display
# props taken off (drop), and given what the story needs: loads and racks,
# an airlock, landing legs, a ramp a player can walk up.

SKETCHFAB = {
    "buggy": "sketchfab:085bd97876a64eacbc041299ac945e9c",  # "Sci - Fi Buggy", TiyaMakes
    "bike": "sketchfab:99290272e69d4cd8a56af2d2e1274e0d",  # "Cyberpunk Bike Concept Design", Berk Gedik
    "hauler": "sketchfab:89ea8e69b87942a881f9b89fd04ec00f",  # "H500 - Explorer Truck", Render Blue
    "rover": "sketchfab:e76d2a9172b54a62b89c9f18710d9cbc",  # Martian Rover "THOTH", Maxim Senchenko
    "drifter": "sketchfab:fe581b60c8ea4ea591a24a483f25ddad",  # "Transporter Spaceship", pirate8888
    "shuttle": "sketchfab:d3e5d9303d1d4e8b8a8e30bf22f59a21",  # "Sci-fi shuttle", Hrvoje Wächter
}


# Where each Sourced vehicle's wheels are, fitted: (centre, radius, width),
# their axles along X, the left (+X) one of each pair first. Their parts are
# cut out of the vehicle and drawn as wheel pieces (polyhaven.wheel_of).
BUGGY_WHEELS = [((x, y, 0.425), 0.43, 0.36) for y in (-1.475, 1.725) for x in (1.285, -1.285)]
BIKE_WHEELS = [((0, y, 0.385), 0.38, 0.22) for y in (-0.715, 0.72)]
HAULER_WHEELS = [((x, y, 0.64), 0.64, 0.5) for y in (-1.635, 0.395, 1.705, 3.025) for x in (1.145, -1.145)]
ROVER_WHEELS = [((x, y, 0.55), 0.545, 0.46) for y in (-1.505, -0.2, 2.65) for x in (1.52, -1.52)]


def buggy(style: Style) -> Piece:
    """A two-seat Fringe runabout: an open wedge tub in a tube cage on long
    travel suspension, its body repainted, a rack bolted over the drive
    with water cans and a crate strapped on. What a Fringer who's done well
    drives."""
    r = rng(style, "buggy")
    paint = r.choice(["Crew orange", "Bleached", "Repaint oxide", "Hazard yellow", "Repaint cream"])
    p = Sourced("buggy", "vehicle", "4.3 × 2.9 × 1.7 m buggy, front at -Y", asset=SKETCHFAB["buggy"], width=4.3,
                budget=40000, paint={"main_body": paint, "rear_strip": "Hazard yellow"}, wheels=BUGGY_WHEELS)
    for i, (c, radius, _) in enumerate(BUGGY_WHEELS):
        front = i < 2
        p.wheel(c, radius, 0.3, "buggy_wheel_front" if front else "buggy_wheel_rear", steer=front, handbrake=not front)
    p.seat((0.4, -0.62, 0.1), exits=[(1.9, -0.4, 0), (-1.9, -0.4, 0), (0, 2.8, 0)])
    p.chassis((2.3, 4.2, 1.1), (0, 0, 0.95))
    p.drive("buggy")
    # The rack over the engine cover, on posts down to it.
    top = 1.38
    p.span((-0.6, 1.0, top), (0.6, 1.85, top + 0.03), "Grating")
    p.tube([(-0.62, 1.0, top + 0.03), (-0.62, 1.87, top + 0.03), (0.62, 1.87, top + 0.03), (0.62, 1.0, top + 0.03)], 0.016,
           "Hull dark", segments=6)
    for x in (-0.55, 0.55):
        for y in (1.05, 1.8):
            p.tube([(x, y, top), (x, y, 1.0)], 0.014, "Hull dark", segments=6)
    for x in (-0.36, -0.06):
        p.source("metal_jerrycan_green", at=(x, 1.42, top + 0.03), rot=(0, 0, 90), height=0.46)
    p.source("plastic_crate_02", at=(0.33, 1.42, top + 0.03), length=0.4)
    p.tube([(-0.62, 1.42, top + 0.03), (-0.5, 1.42, top + 0.5), (0.1, 1.42, top + 0.5), (0.1, 1.42, top + 0.03)], 0.01,
           "Fabric olive", segments=4)
    # A whip aerial off the back of the cage, with a pennant.
    with p.plain():
        p.cyl(0.006, 1.4, (0.7, 0.9, 1.6 + 0.7), "Hull dark", segments=4)
        p.prism([(0.0, 0.0), (0.3, -0.06), (0.0, -0.14)], 0.004, (0.7, 0.9, 2.28), "Fabric red", rot=(0, 0, 90))
    p.collider((2.0, 4.0, 1.0), (0, 0, 0.77))
    return p


def bike(style: Style) -> Piece:
    """A courier's bike: a fat-tyred, stripped-down electric frame with its
    battery and drive out in the open, a courier's bag strapped to the
    tail."""
    p = Sourced("bike", "vehicle", "2.2 × 0.6 × 1.0 m bike, front at -Y", asset=SKETCHFAB["bike"], width=2.2, turns=3,
                budget=30000, wheels=BIKE_WHEELS)
    for i, (c, radius, _) in enumerate(BIKE_WHEELS):
        p.wheel(c, radius, 0.14, "bike_wheel_front" if i == 0 else "bike_wheel_rear", steer=i == 0, drive=i == 1,
                handbrake=i == 1)
    # At the front of the saddle (its top 0.93 m up), up to the tank, the
    # hands on the grips at the ends of the low bars, the feet on the pegs.
    p.seat((0, -0.05, 0.38), exits=[(1.0, 0, 0), (-1.0, 0, 0)], pose="ride",
           grips=[(0.24, -0.36, 0.82), (-0.24, -0.36, 0.82)], pegs=[(0.2, 0.1, 0.32), (-0.2, 0.1, 0.32)])
    p.chassis((0.4, 1.7, 0.6), (0, 0, 0.65))
    p.drive("bike")
    p.span((-0.36, 0.35, 0.52), (-0.24, 0.8, 0.78), "Canvas")
    p.span((-0.37, 0.34, 0.72), (-0.23, 0.81, 0.8), "Fabric olive")
    p.collider((0.45, 2.0, 0.9), (0, 0, 0.5))
    return p


def _hauler(name: str, note: str, style: Style) -> tuple[Piece, "random.Random", str]:
    """The six-axle rover-truck both haulers are: its cargo body repainted,
    its lids in another paint, the bumper in hazard yellow, a rack over its
    roof for what it's carrying."""
    r = rng(style, name)
    paint = r.choice(["Crew orange", "Hazard yellow", "Repaint cream", "Bleached", "Crew orange"])
    patch = r.choice([c for c in REPAINTS if c != paint])
    p = Sourced(name, "vehicle", note, asset=SKETCHFAB["hauler"], width=7.8, stretch=(0.9, 1, 1), budget=60000,
                paint={"trailer": paint, "doors": patch, "front": "Hazard yellow"}, wheels=HAULER_WHEELS)
    # Eight wheels on four axles: the front two steer, the back two take
    # the hand brake.
    for i, (c, radius, _) in enumerate(HAULER_WHEELS):
        p.wheel(c, radius, 0.43, "hauler_wheel", steer=i < 4, handbrake=i >= 4)
    p.seat((0.6, -2.55, 1.35), exits=[(2.2, -2.3, 0), (-2.2, -2.3, 0), (0, -5.0, 0)], pose="inside")
    p.chassis((2.7, 7.8, 2.3), (0, 0, 2.25))
    p.drive("truck")
    z = 3.24
    p.span((-1.05, -1.25, z), (1.05, 2.35, z + 0.05), "Grating")
    for s in (-1, 1):
        p.tube([(s * 1.07, -1.27, z + 0.05), (s * 1.07, -1.27, z + 0.3), (s * 1.07, 2.37, z + 0.3), (s * 1.07, 2.37, z + 0.05)],
               0.025, "Hull dark", segments=6)
    p.collider((2.6, 7.6, 3.0), (0, 0, 1.8))
    return p, r, paint


def hauler(style: Style) -> Piece:
    """The Pads-to-Hull haul contracts' workhorse: an eight-wheeled
    rover-truck, its rack loaded with crates lashed down under a tarp.
    Bought, or given as a faction's favour."""
    p, r, paint = _hauler("hauler", "7.8 × 3.0 × 4.5 m eight-wheel truck, cab at -Y", style)
    z = 3.29
    top = 0
    for y, h in ((-0.7, 0.8), (0.4, 0.9), (1.55, 0.75)):
        for x in (-0.48, 0.48):
            hh = h * r.uniform(0.85, 1.0)
            mat = r.choice(["Crew orange", "Container blue", "Container green", "Repaint cream"])
            p.span((x - 0.45, y - 0.5, z), (x + 0.45, y + 0.5, z + hh), mat)
            p.span((x - 0.47, y - 0.52, z + hh - 0.06), (x + 0.47, y + 0.52, z + hh + 0.012), "Hull dark")
            top = max(top, hh)
    lid = z + top + 0.04
    p.cloth([(-1.0, -1.25, lid), (1.0, -1.25, lid), (1.0, 1.0, lid), (-1.0, 1.0, lid)], "Tarp", sag=-0.04, ripple=0.03,
            thickness=0.012, cell=0.3, seed=3, droop=(0.15, 0.15, 0, 0.15))
    for y in (-0.9, 0.3, 1.5):
        p.tube([(-1.08, y, z + 0.3), (-1.0, y, lid + 0.03), (1.0, y, lid + 0.03), (1.08, y, z + 0.3)], 0.014, "Hazard yellow",
               segments=4)
    p.collider((2.0, 3.5, top), (0, 0.55, z + top / 2))
    p.chassis((2.0, 3.5, top), (0, 0.55, z + top / 2))
    return p


def hauler_tanker(style: Style) -> Piece:
    """The rover-truck with a tank on its rack: water out to the Fringe,
    fuel to the Pads. Banded, a manhole on top, a hose reel and valves at
    the back. Whoever drives it is carrying the colony's leverage."""
    p, r, paint = _hauler("hauler_tanker", "7.8 × 3.0 × 4.9 m eight-wheel tanker, cab at -Y", style)
    z, rad, y0, y1 = 3.29, 0.72, -1.2, 2.3
    zc = z + 0.18 + rad
    p.lathe([(0.0, y0), (0.45, y0 + 0.03), (0.66, y0 + 0.12), (rad, y0 + 0.28), (rad, y1 - 0.28), (0.66, y1 - 0.12), (0.45, y1 - 0.03),
             (0.0, y1)], (0, 0, zc), "Water blue", rot=(-90, 0, 0), segments=16)
    for y in (-0.6, 0.55, 1.7):
        p.torus(rad + 0.012, 0.03, (0, y, zc), "Hull dark", rot=(90, 0, 0), segments=16, sides=5)
        p.span((-0.55, y - 0.08, z), (0.55, y + 0.08, z + 0.3), "Hull dark")
    p.lathe([(0.0, 0.0), (0.22, 0.0), (0.22, 0.1), (0.18, 0.13), (0.0, 0.14)], (0, 0.55, zc + rad - 0.03), "Hull alloy", segments=10)
    p.cyl(0.24, 0.26, (0.65, 2.25, z + 0.35), "Crew orange", rot=(0, 90, 0), segments=12)
    p.cyl(0.19, 0.28, (0.65, 2.25, z + 0.35), "Rubber", rot=(0, 90, 0), segments=12)
    for x in (-0.15, 0.15):
        p.lathe([(0.0, 0.0), (0.04, 0.0), (0.04, 0.16), (0.07, 0.18), (0.07, 0.24), (0.0, 0.24)], (x, y1 - 0.02, zc - 0.35), "Brass",
                rot=(-90, 0, 0), segments=6)
    p.collider((1.4, y1 - y0, 2 * rad), (0, (y0 + y1) / 2, zc))
    p.chassis((1.4, y1 - y0, 2 * rad), (0, (y0 + y1) / 2, zc))
    return p


def rover(style: Style) -> Piece:
    """A crew rover for the long Fringe runs: a six-wheeled hab on rocker
    arms, repainted, portholes down its flanks, an airlock door and steps
    at the back, aerials. Salvage crews and caravan escorts live in these
    for weeks."""
    r = rng(style, "rover")
    paint = r.choice(["Crew orange", "Repaint cream", "Hazard yellow", "Bleached"])
    patch = r.choice([c for c in REPAINTS if c != paint])
    p = Sourced("rover", "vehicle", "6.4 × 3.5 × 3.2 m crew rover, front at -Y", asset=SKETCHFAB["rover"], width=6.4, turns=2,
                stretch=(0.85, 1, 1), budget=60000, drop=("Icosphere",), paint={"marsrider": paint, "devices": patch},
                wheels=ROVER_WHEELS)
    # Six wheels on rocker arms: the front and back pairs steer, the back
    # against the front, to turn tighter.
    for i, (c, radius, _) in enumerate(ROVER_WHEELS):
        p.wheel(c, radius, 0.42, "rover_wheel", steer=i < 2 or i >= 4, handbrake=i >= 4)
    p.seat((0.4, -0.25, 0.95), exits=[(2.4, -0.3, 0), (-2.4, -0.3, 0), (0, 3.9, 0)], pose="inside")
    p.chassis((1.9, 3.7, 1.8), (0, 0.82, 1.5))
    p.chassis((2.2, 4.8, 0.6), (0, 0.5, 1.05))
    p.drive("rover")
    for s in (-1, 1):
        for y in (0.15, 0.85, 1.55):
            p.cyl(0.17, 0.06, (s * 0.93, y, 1.6), "Hull dark", rot=(0, 90, 0), segments=12)
            p.cyl(0.13, 0.07, (s * 0.94, y, 1.6), "Screen", rot=(0, 90, 0), segments=12)
    # The airlock at the back: its frame, the door and its wheel, steps
    # down, handrails.
    p.span((-0.5, 2.6, 0.9), (0.5, 2.68, 2.1), "Hull dark")
    p.span((-0.42, 2.68, 0.97), (0.42, 2.72, 2.03), patch)
    p.torus(0.13, 0.018, (0, 2.74, 1.5), "Steel", rot=(90, 0, 0), segments=12, sides=5)
    for z, y in ((0.66, 2.95), (0.38, 3.2), (0.1, 3.45)):
        p.span((-0.4, y - 0.13, z - 0.03), (0.4, y + 0.13, z), "Grating")
    for s in (-1, 1):
        p.tube([(s * 0.42, 2.7, 0.9), (s * 0.42, 3.55, 0.05)], 0.025, "Hull dark", segments=6)
        p.tube([(s * 0.48, 2.7, 1.8), (s * 0.48, 3.0, 1.5), (s * 0.48, 3.55, 0.85)], 0.018, ACCENT, segments=6)
    with p.plain():
        for x in (-0.75, 0.75):
            p.cyl(0.008, 1.3, (x, 2.4, 2.9), "Hull dark", segments=3)
    p.collider((1.8, 3.5, 1.8), (0, 0.83, 1.5))
    p.collider((2.5, 5.2, 1.1), (0, 0.57, 0.55))  # the body between the wheels
    return p


def receivership_shuttle(style: Style) -> Piece:
    """The Corvane Receivership's lander, white and blue: a cockpit
    fuselage between two great engine pods, down on landing legs, a ramp
    lowered from its belly at the back. Not a scratch on it (CLEAN in
    finish.py keeps it so). It comes down in Act 3 and looks wrong on the
    Red, which is the point."""
    lift = 1.3
    p = Sourced("receivership_shuttle", "vehicle", "Corvane lander, about 15 × 16 × 6.5 m; ramp at the back",
                asset=SKETCHFAB["shuttle"], width=16, turns=2, at=(0, 0, lift), budget=60000, drop=("iznutra", "monitori"),
                paint={"brod_izvana": "Corvane white"})
    # Landing legs: a sleeve, the ram, a broad blue foot.
    for x, y in ((-5.7, -1.4), (5.7, -1.4), (-5.7, 3.2), (5.7, 3.2), (0.0, -5.0)):
        p.cyl(0.2, 0.7, (x, y, lift - 0.25), "Corvane white", segments=12)
        p.cyl(0.11, lift - 0.3, (x, y, 0.2 + (lift - 0.3) / 2), "Steel", segments=10)
        p.span((x - 0.45, y - 0.45, 0.0), (x + 0.45, y + 0.45, 0.18), "Corvane blue")
        p.collider((0.9, 0.9, lift), (x, y, lift / 2))
    # The ramp, down from the belly behind the cockpit to the ground at the
    # back, its rails and the hatch's dark well.
    hinge_y, hinge_z, run, w = 3.4, 1.55, 3.4, 2.0
    a = math.atan2(hinge_z, run)
    length = math.hypot(run, hinge_z)
    t = 0.12
    cy, cz = hinge_y + run / 2, hinge_z / 2 - t / 2
    p.box((w, length, t), (0, cy, cz), "Corvane white", rot=(-math.degrees(a), 0, 0))
    p.collider((w, length, t), (0, cy, cz), rot=(-math.degrees(a), 0, 0))
    for s in (-1, 1):
        p.box((0.08, length, 0.1), (s * w / 2, cy, cz + 0.08), "Corvane blue", rot=(-math.degrees(a), 0, 0))
        p.tube([(s * (w / 2 - 0.05), hinge_y, hinge_z + 0.9), (s * (w / 2 - 0.05), hinge_y + run - 0.3, 0.95)], 0.03, "Steel",
               segments=6)
    p.span((-1.0, hinge_y - 1.6, hinge_z - 0.05), (1.0, hinge_y, hinge_z + 0.02), "Hull dark")
    p.collider((3.8, 11.0, 3.0), (0, -1.8, lift + 1.8))
    for s in (-1, 1):
        p.collider((3.4, 6.5, 4.4), (s * 5.71, 0.88, lift + 2.25))
    return p


def drifter_patience(style: Style) -> Piece:
    """The Patience: an independent long-haul drifter, landed on its legs at
    the Pads with its hold open and the ramp down. Every player steps off
    it, in debt. Forty years of repairs show: its hull repainted grey, its
    armour Crew orange, its guns long since sold. Cargo waits in the hold."""
    # The hold's numbers, measured off the model as it's fitted here.
    floor, mouth, back, aft, top, belly, side = 4.17, -13.95, 1.29, 11.7, 10.3, 1.55, 4.45
    p = Sourced("drifter_patience", "vehicle", "drifter freighter, about 25 × 29 × 10 m; walk up the ramp into the hold",
                asset=SKETCHFAB["drifter"], width=28.9, turns=2, budget=150000, drop=("pregun", "gunsteel", "hair", "_ramp_", "cube.020_steeldirty"),
                paint={"mainhull": "Hull pale", "armor": "Crew orange"})
    r = rng(style, "drifter_patience")
    # The ramp, down from the hold's lip; it replaces the model's, which
    # was left level.
    _ramp(p, mouth, floor, 7.0, 7.0, "Hull alloy")
    # Cargo in the hold: crates against the back wall, a pallet under a net,
    # the ship's registration on a plate (it was never given letters).
    for x in (-2.6, 0.0, 2.6):
        mat = r.choice(["Crew orange", "Container blue", "Container green"])
        p.span((x - 0.9, back - 2.0, floor), (x + 0.9, back - 0.3, floor + 1.4), mat)
        p.span((x - 0.92, back - 2.02, floor + 1.34), (x + 0.92, back - 0.28, floor + 1.412), "Hull dark")
    p.span((-3.2, -6.0, floor), (-1.8, -4.6, floor + 0.15), "Wood")
    p.cloth([(-3.2, -6.0, floor + 0.9), (-1.8, -6.0, floor + 0.9), (-1.8, -4.6, floor + 0.9), (-3.2, -4.6, floor + 0.9)], "Tarp",
            sag=-0.05, ripple=0.04, thickness=0.012, cell=0.25, seed=7, droop=(0.7, 0.7, 0.7, 0.7))
    p.span((-0.8, back - 0.3, floor + 1.9), (0.8, back - 0.26, floor + 2.4), "Hull dark")
    with p.painted():
        p.stencil("60-114", (0, back - 0.306, floor + 2.15), 0.3, "Stencil white", facing="-Y")
    # Colliders: the hold's floor, walls and roof, the hull round it and
    # behind it, the feet.
    hy0, hy1 = mouth, back
    p.collider((2 * side - 0.1, hy1 - hy0, floor - belly), (0, (hy0 + hy1) / 2, (belly + floor) / 2))
    for s in (-1, 1):
        p.collider((12.3 - side, aft - hy0, top - belly), (s * (side + 12.3) / 2, (hy0 + aft) / 2, (belly + top) / 2))
    p.collider((2 * side, hy1 - hy0, top - 7.21), (0, (hy0 + hy1) / 2, (7.21 + top) / 2))
    p.collider((2 * side, aft - hy1, top - belly), (0, (hy1 + aft) / 2, (belly + top) / 2))
    for x, y, sx, sy in ((-6.53, -10.1, 2.8, 1.5), (6.58, -10.1, 2.8, 1.5), (0.0, 4.0, 1.4, 3.5)):
        p.collider((sx, sy, 0.3), (x, y, 0.15))
        p.collider((0.8, 0.8, belly), (x, y, belly / 2))
    return p


def covered_car(style: Style) -> Piece:
    """A car under a tarp, parked and left: someone's Earth runabout, kept
    for a better day. Dressing for the Pads and the Fringe. (Poly Haven's
    covered_car.)"""
    p = Sourced("covered_car", "prop", "car under a tarp, 1.8 × 4.4 × 1.4 m", asset="covered_car", res="2k", width=4.4, budget=20000)
    p.collider((1.7, 4.2, 1.3), (0, 0, 0.65))
    return p


def buggy_wheel_front(style: Style) -> Piece:
    return wheel_of(buggy(style), "buggy_wheel_front", 0)


def buggy_wheel_rear(style: Style) -> Piece:
    return wheel_of(buggy(style), "buggy_wheel_rear", 2)


def bike_wheel_front(style: Style) -> Piece:
    return wheel_of(bike(style), "bike_wheel_front", 0)


def bike_wheel_rear(style: Style) -> Piece:
    return wheel_of(bike(style), "bike_wheel_rear", 1)


def hauler_wheel(style: Style) -> Piece:
    """The haulers' wheel: the tanker's are the same."""
    return wheel_of(hauler(style), "hauler_wheel", 0)


def rover_wheel(style: Style) -> Piece:
    return wheel_of(rover(style), "rover_wheel", 0)


PIECES = [hauler, hauler_tanker, bike, trike, buggy, rover, drifter_patience, receivership_shuttle, covered_car,
          buggy_wheel_front, buggy_wheel_rear, bike_wheel_front, bike_wheel_rear, hauler_wheel, rover_wheel,
          trike_wheel_front, trike_wheel_rear]
