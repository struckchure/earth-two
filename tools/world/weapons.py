"""Weapons: anything money can buy (docs/economy.md). Earth guns and their
ammunition come only on drifters; laser guns burn power cells; blades are
cheap and never run out; batons and armour are what guards carry.

What exists on Earth comes from Poly Haven's models (polyhaven.py): the
pistol, the bolt-action rifle, the katana, the machete, the ammunition can.
The rest is modelled: each gun drawn as a side view (its profile, cut from
plate, with round barrels and rods) and laid on its side as on a table,
muzzle or tip along +X, top towards -Y, its origin in the middle of where it
lies. The rack stands them up instead.

Parts of different colours never share a face plane (see GUIDE.md): a
plate laid over another is 12 mm thicker (6 mm proud each side), a ring
round a rod 6 mm wider, and anything thinner is paint."""
import math

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

import polyhaven
from kit import PALETTE, Piece, Style, rng
from polyhaven import Sourced

CATEGORY = "Weapons"

PALETTE.setdefault("Polymer", (0.06, 0.06, 0.05))
PALETTE.setdefault("Olive drab", (0.10, 0.11, 0.05))
PALETTE.setdefault("Lacquer black", (0.015, 0.012, 0.012))
PALETTE.setdefault("Wrap blue", (0.03, 0.05, 0.12))
PALETTE.setdefault("Blade steel", (0.62, 0.64, 0.66))

# How much thicker a plate laid over another is (6 mm proud each side), and
# how much wider a ring round a rod: clear of the depth buffer's reach.
PROUD = 0.012


class Cropped(Sourced):
    """A Sourced piece cut down to the parts of its model keep is true of:
    keep(lo, hi) is a test on each connected part's bounds, in the model's
    own frame before it's fitted. One pistol from a set of two, without its
    spare magazine."""

    def __init__(self, *args, keep, **kwargs):
        super().__init__(*args, **kwargs)
        self.keep = keep

    def build(self, collection=None):
        real = polyhaven.load

        def load(asset, res="2k"):
            o = real(asset, res)
            bm = bmesh.new()
            bm.from_mesh(o.data)
            drop, seen = [], set()
            for v in bm.verts:
                if v in seen:
                    continue
                part, stack = [], [v]
                seen.add(v)
                while stack:
                    a = stack.pop()
                    part.append(a)
                    for e in a.link_edges:
                        b = e.other_vert(a)
                        if b not in seen:
                            seen.add(b)
                            stack.append(b)
                lo = Vector([min(p.co[i] for p in part) for i in range(3)])
                hi = Vector([max(p.co[i] for p in part) for i in range(3)])
                if not self.keep(lo, hi):
                    drop += part
            bmesh.ops.delete(bm, geom=drop, context="VERTS")
            bm.to_mesh(o.data)
            bm.free()
            return o

        polyhaven.load = load
        try:
            return super().build(collection)
        finally:
            polyhaven.load = real


class Profile:
    """A weapon in its own side view: x along it (muzzle +X), z up its
    profile, y through it. Parts are plates cut to an outline, rods along
    it and rings; emit() places the lot."""

    def __init__(self) -> None:
        self.parts = []  # (kind, args, local Matrix)
        self.lo = Vector((math.inf,) * 3)
        self.hi = Vector((-math.inf,) * 3)

    def _grow(self, lo, hi) -> None:
        self.lo = Vector(map(min, self.lo, lo))
        self.hi = Vector(map(max, self.hi, hi))

    def plate(self, outline, thick: float, mat: str, y: float = 0.0) -> None:
        """A plate cut to outline ((x, z) points), thick through, at y."""
        self.parts.append(("prism", (outline, thick, mat), Matrix.Translation((0, y, 0))))
        xs, zs = [p[0] for p in outline], [p[1] for p in outline]
        self._grow((min(xs), y - thick / 2, min(zs)), (max(xs), y + thick / 2, max(zs)))

    def rect(self, x0, z0, x1, z1, thick: float, mat: str, y: float = 0.0) -> None:
        self.plate([(x0, z0), (x1, z0), (x1, z1), (x0, z1)], thick, mat, y)

    def rod(self, x0, x1, z, radius, mat: str, y: float = 0.0, segments: int = 7, radius2=None) -> None:
        """A round rod (a barrel, a tube, a grip) from x0 to x1 at height z."""
        m = Matrix.Translation(((x0 + x1) / 2, y, z)) @ Euler((0, math.pi / 2, 0)).to_matrix().to_4x4()
        self.parts.append(("cyl", (radius, x1 - x0, mat, segments, radius2), m))
        r = max(radius, radius2 or 0)
        self._grow((x0, y - r, z - r), (x1, y + r, z + r))

    def turned(self, x0, profile, z, mat: str, y: float = 0.0, segments: int = 7) -> None:
        """A turned part along x from x0: profile is (radius, length along)."""
        m = Matrix.Translation((x0, y, z)) @ Euler((0, math.pi / 2, 0)).to_matrix().to_4x4()
        self.parts.append(("lathe", (profile, mat, segments), m))
        r = max(p[0] for p in profile)
        self._grow((x0, y - r, z - r), (x0 + max(p[1] for p in profile), y + r, z + r))

    def ring(self, x, z, major, minor, mat: str, arc=360.0, start=0.0, y: float = 0.0) -> None:
        """A ring in the profile's plane round (x, z): arc degrees from start
        (0 is +X, 90 is up)."""
        m = Matrix.Translation((x, y, z)) @ Euler((math.pi / 2, 0, 0)).to_matrix().to_4x4() \
            @ Euler((0, 0, math.radians(start))).to_matrix().to_4x4()
        self.parts.append(("torus", (major, minor, mat, arc), m))
        self._grow((x - major - minor, y - minor, z - major - minor), (x + major + minor, y + minor, z + major + minor))

    def pin(self, x, z, radius, thick: float, mat: str) -> None:
        """A pin or screw head through the profile, showing both sides."""
        m = Matrix.Translation((x, 0, z)) @ Euler((math.pi / 2, 0, 0)).to_matrix().to_4x4()
        self.parts.append(("cyl", (radius, thick, mat, 5, None), m))

    def paint(self, outline, thick: float, mat: str) -> None:
        """Paint on both faces of a plate thick through: 5 mm proud of it."""
        self.parts.append(("paint", (outline, thick + 0.01, mat), Matrix.Identity(4)))

    def emit(self, p: Piece, place: Matrix) -> None:
        for kind, args, local in self.parts:
            m = place @ local
            loc, rot, _ = m.decompose()
            e = [math.degrees(a) for a in rot.to_euler()]
            if kind == "prism":
                outline, thick, mat = args
                p.prism(outline, thick, loc, mat, rot=e)
            elif kind == "paint":
                outline, thick, mat = args
                with p.painted():
                    p.prism(outline, thick, loc, mat, rot=e)
            elif kind == "cyl":
                radius, depth, mat, segments, radius2 = args
                p.cyl(radius, depth, loc, mat, rot=e, segments=segments, radius2=radius2)
            elif kind == "lathe":
                profile, mat, segments = args
                p.lathe([(r, z) for r, z in profile], loc, mat, rot=e, segments=segments)
            elif kind == "torus":
                major, minor, mat, arc = args
                p.torus(major, minor, loc, mat, rot=e, segments=7, sides=5, arc=arc)

    def lay_flat(self, p: Piece) -> None:
        """On its side, as on a table: its middle on the origin, top to -Y."""
        c = (self.lo + self.hi) / 2
        turn = Euler((math.pi / 2, 0, 0)).to_matrix().to_4x4()
        place = Matrix.Translation((-c.x, c.z, -self.lo.y)) @ turn
        self.emit(p, place)

    def stand_up(self, p: Piece, at, facing: float = 0.0) -> None:
        """Upright, muzzle up, its butt on at, turned facing degrees about Z."""
        c = (self.lo + self.hi) / 2
        up = Euler((0, -math.pi / 2, 0)).to_matrix().to_4x4()  # +X to +Z
        place = Matrix.Translation(at) @ Euler((0, 0, math.radians(facing))).to_matrix().to_4x4() @ up \
            @ Matrix.Translation((-self.lo.x, -c.y, -c.z))
        self.emit(p, place)


# The modelled guns' profiles, shared by the guns themselves and the rack.

def _shotgun() -> Profile:
    """A pump gun: walnut stock and pump, a blued receiver, a magazine tube
    under the barrel held by a band."""
    g = Profile()
    t = 0.04
    # The stock, its grip swept down, a rubber pad on the butt.
    g.plate([(-0.47, -0.085), (-0.465, 0.03), (-0.2, 0.045), (-0.17, 0.02), (-0.21, -0.005),
             (-0.25, -0.03), (-0.3, -0.035), (-0.42, -0.095)], t, "Wood dark")
    g.plate([(-0.49, -0.095), (-0.468, -0.088), (-0.463, 0.033), (-0.488, 0.036)], t + PROUD, "Rubber")
    for z in (-0.06, -0.02, 0.015):
        g.rect(-0.492, z, -0.488, z + 0.02, t + PROUD, "Polymer")
    # The receiver, with its ejection port, pins and a rail on top.
    g.plate([(-0.2, -0.008), (0.03, -0.008), (0.03, 0.06), (-0.19, 0.066), (-0.205, 0.05)], t - PROUD, "Gunmetal")
    g.paint([(-0.12, 0.02), (-0.05, 0.02), (-0.05, 0.045), (-0.12, 0.045)], t - PROUD, "Hull dark")
    for x in (-0.17, -0.02):
        g.pin(x, 0.008, 0.005, t - PROUD + 0.012, "Steel")
    g.rect(-0.16, 0.066, 0.0, 0.074, 0.016, "Gunmetal")
    # Trigger guard and trigger.
    g.ring(-0.125, -0.008, 0.028, 0.004, "Gunmetal", arc=180, start=180)
    g.plate([(-0.13, -0.008), (-0.12, -0.008), (-0.123, -0.03), (-0.132, -0.028)], 0.007, "Steel")
    # Barrel and magazine tube, the band between, the bead sight.
    g.turned(0.03, [(0.0, 0.0), (0.014, 0.0), (0.014, 0.44), (0.0, 0.44)], 0.045, "Gunmetal")
    g.turned(0.03, [(0.0, 0.0), (0.012, 0.0), (0.012, 0.4), (0.0, 0.4)], 0.016, "Gunmetal")
    g.rect(0.39, -0.004, 0.41, 0.066, 0.036, "Steel")
    g.turned(0.462, [(0.0, 0.0), (0.004, 0.0), (0.004, 0.01), (0.0, 0.012)], 0.061, "Brass", segments=5)
    # The pump: ridged walnut round the magazine tube.
    g.turned(0.1, [(0.0, 0.0), (0.024, 0.0), (0.027, 0.01), (0.027, 0.15), (0.024, 0.16), (0.0, 0.16)], 0.017, "Wood")
    for x in (0.13, 0.18, 0.23):
        g.turned(x, [(0.0, 0.0), (0.033, 0.0), (0.033, 0.012), (0.0, 0.012)], 0.017, "Wood dark")
    return g


def _laser_pistol() -> Profile:
    """A laser pistol: a blocky white housing over a dark frame, cooling
    fins along the top, a finned emitter, and a grip that's its power cell.
    Well made somewhere else; the tape round its middle was put on here."""
    g = Profile()
    t = 0.042
    g.plate([(-0.09, 0.0), (0.06, 0.0), (0.075, 0.015), (0.075, 0.07), (-0.08, 0.075), (-0.095, 0.05)], t, "Corvane white")
    g.plate([(-0.085, -0.008), (0.068, -0.008), (0.068, 0.012), (-0.085, 0.012)], t - PROUD, "Gunmetal")
    # The vent: a dark window with cyan glowing between its fins.
    g.rect(-0.06, 0.025, 0.04, 0.055, t + PROUD, "Gunmetal")
    for i in range(6):
        x = -0.055 + i * 0.016
        g.rect(x, 0.031, x + 0.008, 0.049, t + 2 * PROUD, "Laser cyan" if i % 2 else "Hull dark")
    # Cooling fins along the top.
    for i in range(7):
        x = -0.07 + i * 0.018
        g.rect(x, 0.075, x + 0.006, 0.084, t - 2 * PROUD, "Gunmetal")
    # The emitter: a stepped nozzle with fins, and the lens.
    g.turned(0.075, [(0.0, 0.0), (0.02, 0.0), (0.02, 0.02), (0.016, 0.022), (0.016, 0.048), (0.0, 0.048)], 0.04, "Gunmetal")
    for x in (0.08, 0.092, 0.104):
        g.turned(x, [(0.0, 0.0), (0.028, 0.0), (0.028, 0.004), (0.0, 0.004)], 0.04, "Hull dark")
    g.turned(0.123, [(0.0, 0.0), (0.011, 0.0), (0.009, 0.006), (0.0, 0.007)], 0.04, "Laser cyan")
    # The grip is the cell: polymer, a brass base, a cyan charge window.
    g.plate([(-0.075, 0.0), (-0.03, 0.0), (-0.045, -0.085), (-0.085, -0.08)], t, "Polymer")
    g.plate([(-0.07, -0.08), (-0.044, -0.082), (-0.046, -0.1), (-0.072, -0.098)], t - PROUD, "Brass")
    g.rect(-0.068, -0.062, -0.052, -0.022, t + PROUD, "Laser cyan")
    for z in (-0.072, -0.018):
        g.plate([(-0.083, z), (-0.035, z), (-0.036, z + 0.006), (-0.082, z + 0.006)], t + PROUD, "Hull dark")
    g.ring(-0.005, 0.0, 0.02, 0.0035, "Gunmetal", arc=180, start=180)
    g.plate([(-0.012, 0.0), (-0.004, 0.0), (-0.008, -0.016), (-0.014, -0.014)], 0.006, "Steel")
    # Patched: Crew tape wound round the housing where it cracked.
    g.rect(0.048, -0.016, 0.057, 0.081, t + PROUD, "Crew orange")
    g.pin(-0.075, 0.035, 0.004, t + 0.012, "Steel")
    return g


def _laser_rifle() -> Profile:
    """A laser rifle: a long white housing on a dark frame, a finned barrel
    shroud, a cell slung under the emitter, a skeleton stock."""
    g = Profile()
    t = 0.05
    # Skeleton stock: an open frame with a rubber pad.
    g.plate([(-0.48, -0.06), (-0.47, 0.04), (-0.3, 0.05), (-0.3, 0.0), (-0.36, -0.01), (-0.42, -0.06)], t - PROUD, "Polymer")
    g.plate([(-0.44, -0.035), (-0.41, -0.035), (-0.37, 0.018), (-0.42, 0.024)], t, "Corvane white")
    g.plate([(-0.495, -0.066), (-0.476, -0.062), (-0.468, 0.045), (-0.49, 0.047)], t, "Rubber")
    # The housing over its frame.
    g.plate([(-0.3, -0.01), (0.25, -0.01), (0.3, 0.02), (0.3, 0.075), (-0.3, 0.08)], t, "Corvane white")
    g.plate([(-0.29, -0.016), (0.24, -0.016), (0.24, 0.0), (-0.29, 0.0)], t - PROUD, "Gunmetal")
    g.rect(-0.25, 0.08, 0.2, 0.094, t - 2 * PROUD, "Gunmetal")
    for i in range(5):
        x = -0.22 + i * 0.09
        g.rect(x, 0.094, x + 0.016, 0.104, t - 2 * PROUD, "Gunmetal")
    # The charge vents: dark windows with cyan between their fins.
    for x0 in (-0.2, 0.0):
        g.rect(x0, 0.018, x0 + 0.16, 0.06, t + PROUD, "Gunmetal")
        for i in range(3):
            x = x0 + 0.02 + i * 0.05
            g.rect(x, 0.024, x + 0.02, 0.054, t + 2 * PROUD, "Laser cyan" if i % 2 == 0 else "Hull dark")
    # Barrel shroud, finned, and the lens.
    g.turned(0.3, [(0.0, 0.0), (0.022, 0.0), (0.022, 0.18), (0.0, 0.18)], 0.045, "Gunmetal")
    for x in (0.33, 0.39, 0.45):
        g.turned(x, [(0.0, 0.0), (0.032, 0.0), (0.032, 0.014), (0.0, 0.014)], 0.045, "Hull dark")
    g.turned(0.48, [(0.0, 0.0), (0.018, 0.0), (0.014, 0.01), (0.0, 0.012)], 0.045, "Laser cyan")
    # The cell slung under it, in a cradle; the grip and trigger behind.
    g.turned(-0.05, [(0.0, 0.0), (0.026, 0.0), (0.026, 0.27), (0.0, 0.27)], -0.036, "Gunmetal")
    g.turned(-0.06, [(0.0, 0.0), (0.032, 0.0), (0.032, 0.025), (0.0, 0.025)], -0.036, "Brass")
    for x in (0.06, 0.12):
        g.turned(x, [(0.0, 0.0), (0.032, 0.0), (0.032, 0.016), (0.0, 0.016)], -0.036, "Laser cyan")
    g.rect(0.0, -0.016, 0.02, -0.01, 0.03, "Gunmetal")
    g.plate([(-0.29, -0.01), (-0.24, -0.01), (-0.255, -0.1), (-0.295, -0.095)], t, "Polymer")
    g.ring(-0.215, -0.01, 0.025, 0.004, "Gunmetal", arc=180, start=180)
    g.plate([(-0.222, -0.01), (-0.212, -0.01), (-0.215, -0.03), (-0.224, -0.028)], 0.007, "Steel")
    # A strip of Crew tape round the housing.
    g.rect(-0.034, -0.022, -0.016, 0.086, t + 2 * PROUD, "Crew orange")
    for x in (-0.28, 0.22):
        g.pin(x, 0.03, 0.004, t + 0.012, "Steel")
    return g


def earth_pistol(style: Style) -> Piece:
    """A real Earth pistol, decades old and still good: a worn steel slide,
    wooden grips. Earth guns come only on drifters. (Poly Haven's
    service_pistol: the assembled one of its pair, without the spare
    magazine.)"""
    return Cropped("earth_pistol", "item", "pistol, 0.2 m long", asset="service_pistol", res="1k",
                   rot=(90, 0, 0), length=0.2,
                   # The assembled pistol (the set's other is field-stripped), not its magazine.
                   keep=lambda lo, hi: lo.z > 0.075 and lo.x > -0.065)


def earth_rifle(style: Style) -> Piece:
    """A bolt-action rifle from Earth, its walnut stock worn pale where it's
    held. (Poly Haven's bolt_action_rifle_7_62, laid on its side.)"""
    return Sourced("earth_rifle", "item", "bolt-action rifle, 1.06 m long", asset="bolt_action_rifle_7_62",
                   res="1k", rot=(90, 0, 0), length=1.06)


def shotgun(style: Style) -> Piece:
    """A pump shotgun: the guard's favourite for close work in the Hull."""
    p = Piece("shotgun", "item", "pump shotgun, 0.97 m long")
    _shotgun().lay_flat(p)
    return p


def laser_pistol(style: Style) -> Piece:
    """A laser pistol: a chunky finned emitter, its grip the power cell it
    burns. Well made somewhere else, patched here."""
    p = Piece("laser_pistol", "item", "laser pistol, 0.23 m long")
    _laser_pistol().lay_flat(p)
    return p


def laser_rifle(style: Style) -> Piece:
    """A laser rifle: a long emitter, cyan vents, a cell slung under it."""
    p = Piece("laser_rifle", "item", "laser rifle, 0.98 m long")
    _laser_rifle().lay_flat(p)
    return p


def katana(style: Style) -> Piece:
    """A katana beside its saya: an old blade (Poly Haven's
    antique_katana_01) and a black lacquered scabbard with brass fittings
    and a blue cord, made for it here."""
    p = Sourced("katana", "item", "katana and saya, 1.0 m long", asset="antique_katana_01", res="1k",
                rot=(0, 90, 0), length=1.0)
    # The saya, alongside towards +Y: a flattened tube with a little curve,
    # its mouth (koiguchi) and tip (kojiri) in brass.
    y, z = 0.085, 0.016
    pts = [(-0.18 + 0.68 * i / 10, y + 0.012 * math.sin(math.pi * i / 10), z) for i in range(11)]
    p.tube(pts, 0.016, "Lacquer black", segments=5)
    p.cyl(0.019, 0.03, (pts[0][0] + 0.005, pts[0][1], z), "Brass", rot=(0, 90, 0), segments=6)
    p.cyl(0.019, 0.035, (pts[-1][0], pts[-1][1], z), "Brass", rot=(0, 90, 0), segments=6, radius2=0.012)
    # The sageo cord, wound once round near the mouth and trailing.
    p.torus(0.022, 0.004, (pts[0][0] + 0.09, y, z), "Wrap blue", rot=(0, 90, 0), segments=6, sides=5)
    p.cable((pts[0][0] + 0.09, y + 0.022, z), (pts[0][0] + 0.2, y + 0.07, 0.004), 0.004, "Wrap blue", sag=0.0, segments=5, steps=3)
    return p


def machete(style: Style) -> Piece:
    """A machete: the Fringe's tool for everything, fighting included.
    (Poly Haven's machete.)"""
    return Sourced("machete", "item", "machete, 0.65 m long", asset="machete", res="1k", rot=(0, 90, 0), length=0.65)


def knife(style: Style) -> Piece:
    """A combat knife: a clip point with a fuller, a steel guard, a ribbed
    rubber grip and a lanyard hole in the pommel."""
    p = Piece("knife", "item", "knife, 0.31 m long")
    g = Profile()
    t = 0.005
    g.plate([(0.0, -0.017), (0.11, -0.017), (0.15, -0.01), (0.175, 0.002), (0.14, 0.011), (0.115, 0.019),
             (0.0, 0.021)], t, "Blade steel")
    # The fuller, as paint.
    g.paint([(0.02, 0.006), (0.1, 0.008), (0.1, 0.012), (0.02, 0.012)], t, "Gunmetal")
    # Guard, grip, pommel.
    g.plate([(-0.012, -0.03), (0.0, -0.028), (0.0, 0.034), (-0.012, 0.032)], 0.024, "Gunmetal")
    grip = [(0.0, 0.0), (0.014, 0.0)]
    for i in range(6):  # ribbed: ridges and grooves round it
        x = 0.012 + i * 0.016
        grip += [(0.016, x), (0.016, x + 0.008), (0.0135, x + 0.01), (0.0135, x + 0.016)]
    grip += [(0.015, 0.11), (0.014, 0.118), (0.0, 0.118)]
    g.turned(-0.13, grip, 0.002, "Rubber", segments=6)
    g.turned(-0.145, [(0.0, 0.0), (0.014, 0.0), (0.018, 0.006), (0.018, 0.015), (0.0, 0.016)], 0.002, "Gunmetal", segments=6)
    g.ring(-0.15, 0.002, 0.008, 0.002, "Steel")
    g.lay_flat(p)
    return p


def stun_baton(style: Style) -> Piece:
    """A stun baton: a ribbed rubber grip with its switch, a steel shaft,
    and two cyan contacts at the tip. What Registrar guards carry on the
    Exchange floor."""
    p = Piece("stun_baton", "item", "stun baton, 0.6 m long")
    g = Profile()
    grip = [(0.0, 0.0), (0.02, 0.0), (0.02, 0.01), (0.017, 0.012)]
    for i in range(7):  # ribbed rubber
        x = 0.02 + i * 0.02
        grip += [(0.017, x), (0.0195, x + 0.004), (0.0195, x + 0.012), (0.017, x + 0.016)]
    grip += [(0.017, 0.17), (0.0, 0.17)]
    g.turned(-0.3, grip, 0.0, "Rubber")
    g.ring(-0.31, 0.0, 0.012, 0.003, "Steel")
    # The collar with the switch and its charge light.
    g.turned(-0.13, [(0.0, 0.0), (0.027, 0.0), (0.027, 0.03), (0.0, 0.03)], 0.0, "Gunmetal")
    g.rect(-0.125, 0.024, -0.105, 0.034, 0.016, "Hazard yellow")
    g.rect(-0.122, -0.034, -0.108, -0.024, 0.012, "Laser cyan")
    # The shaft, with a guard ring, and the head.
    g.turned(-0.1, [(0.0, 0.0), (0.013, 0.0), (0.013, 0.34), (0.0, 0.34)], 0.0, "Steel")
    g.turned(-0.09, [(0.0, 0.0), (0.03, 0.0), (0.03, 0.008), (0.0, 0.008)], 0.0, "Gunmetal")
    g.turned(0.24, [(0.0, 0.0), (0.019, 0.0), (0.021, 0.01), (0.021, 0.03), (0.016, 0.04), (0.0, 0.04)], 0.0, "Gunmetal")
    for z in (-0.009, 0.009):
        g.rod(0.27, 0.3, z, 0.004, "Laser cyan", segments=5)
    g.lay_flat(p)
    return p


def armour_vest(style: Style) -> Piece:
    """A plate carrier on a stand: front and back plates in their bags,
    padded shoulder straps, a cummerbund, rows of webbing, magazine and
    radio pouches with buckled flaps, a drag handle and a name tape. Sold
    at the Pads on drifter days."""
    p = Piece("armour_vest", "item", "plate carrier on a stand, 0.46 × 0.4 × 1.4 m")
    r = rng(style, p.name)
    cloth = r.choice(["Olive drab", "Crew grey", "Repaint navy"])
    trim = "Polymer"
    base = 0.85
    # The stand: a weighted foot, a post, and shoulders to hang it on.
    p.lathe([(0.0, 0.0), (0.18, 0.0), (0.18, 0.025), (0.12, 0.05), (0.03, 0.06), (0.0, 0.06)], (0, 0, 0), "Hull dark", segments=9)
    p.cyl(0.02, base + 0.48, (0, 0, (base + 0.48) / 2 + 0.03), "Steel", segments=5)
    p.tube([(-0.17, 0, base + 0.42), (-0.12, 0, base + 0.5), (0.12, 0, base + 0.5), (0.17, 0, base + 0.42)], 0.012, "Hull dark", segments=5)
    # The plate bags, front and back, cut to a chest.
    chest = [(-0.17, 0.0), (0.17, 0.0), (0.175, 0.3), (0.15, 0.38), (0.1, 0.46), (0.06, 0.46), (0.045, 0.42),
             (-0.045, 0.42), (-0.06, 0.46), (-0.1, 0.46), (-0.15, 0.38), (-0.175, 0.3)]
    p.prism(chest, 0.05, (0, -0.06, base), cloth, rot=(0, 0, 0))
    p.prism(chest, 0.045, (0, 0.065, base), cloth)
    # Padded shoulder straps over the top, buckled at the front.
    for x in (-0.08, 0.08):
        p.cable((x, -0.085, base + 0.43), (x, 0.088, base + 0.43), 0.022, cloth, sag=-0.07, segments=5, steps=2)
        p.span((x - 0.018, -0.105, base + 0.36), (x + 0.018, -0.093, base + 0.4), trim)
    # The cummerbund round the sides.
    for s in (-1, 1):
        p.span((s * 0.175 - 0.025, -0.075, base + 0.03), (s * 0.175 + 0.025, 0.08, base + 0.23), cloth)
    # Webbing: rows of loops across the front, above the pouches (the rows
    # behind them don't show).
    for z in (0.24, 0.3):
        p.span((-0.16, -0.091, base + z), (0.16, -0.079, base + z + 0.025), cloth)
        for x in (-0.12, 0.12):
            p.span((x - 0.006, -0.097, base + z - 0.006), (x + 0.006, -0.085, base + z + 0.031), trim)
    # Magazine pouches: three, their flaps buckled.
    pouch = "Olive drab" if cloth != "Olive drab" else "Crew grey"
    for i in range(3):
        x = -0.1 + i * 0.1
        p.span((x - 0.042, -0.135, base + 0.04), (x + 0.042, -0.091, base + 0.2), pouch)
        p.span((x - 0.046, -0.149, base + 0.16), (x + 0.046, -0.089, base + 0.21), pouch)
        p.span((x - 0.012, -0.161, base + 0.13), (x + 0.012, -0.143, base + 0.17), trim)
    # A radio pouch on the left shoulder, its antenna up.
    p.span((-0.172, -0.13, base + 0.27), (-0.106, -0.091, base + 0.38), pouch)
    p.cyl(0.004, 0.22, (-0.13, -0.11, base + 0.49), "Polymer", segments=5)
    # The drag handle at the back's top, and the name tape.
    p.cable((-0.04, 0.09, base + 0.4), (0.04, 0.09, base + 0.4), 0.01, trim, sag=-0.04, segments=5, steps=2)
    with p.painted():
        p.span((-0.048, -0.091, base + 0.355), (0.048, -0.089, base + 0.39), "Bleached")
        p.stencil("47", (0, -0.095, base + 0.372), 0.024, "Polymer")
    return p


def ammo_box(style: Style) -> Piece:
    """A steel ammunition can from Earth, its lid latched, still marked
    for the 7.62 rounds it came with. (Poly Haven's ammo_box.)"""
    return Sourced("ammo_box", "item", "ammo can, 0.28 × 0.09 × 0.19 m", asset="ammo_box", res="1k", turns=1, length=0.28)


def laser_cell_pack(style: Style) -> Piece:
    """Three spare power cells in a webbing strap: what a laser burns.
    Each a gunmetal cylinder with grip grooves, a cyan charge ring and brass
    terminals."""
    p = Piece("laser_cell_pack", "item", "three cells, 0.2 × 0.08 × 0.08 m")
    for i in range(3):
        x = -0.062 + i * 0.062
        p.lathe([(0.0, 0.0), (0.025, 0.0), (0.025, 0.062), (0.016, 0.07), (0.0, 0.07)], (x, 0, 0), "Gunmetal", segments=7)
        p.cyl(0.0285, 0.008, (x, 0, 0.045), "Laser cyan", segments=7)
        p.cyl(0.031, 0.012, (x, 0, 0.012), "Corvane white", segments=7)
        p.cyl(0.008, 0.012, (x, 0, 0.074), "Brass", segments=5)
    # The strap round them, and its buckle.
    p.span((-0.1, -0.033, 0.03), (0.1, 0.033, 0.042), "Olive drab")
    p.span((0.085, -0.039, 0.024), (0.105, 0.039, 0.054), "Polymer")
    with p.painted():
        p.stencil("3", (-0.03, -0.0365, 0.036), 0.009, "Stencil white")
    return p


def weapon_rack(style: Style) -> Piece:
    """A weapons dealer's stand: a pegboard on a steel frame, rifles and a
    shotgun stood in it, laser guns hung on hooks, ammunition cans on the
    shelf, and price tags on everything."""
    p = Piece("weapon_rack", "prop", "1.4 × 0.5 × 1.9 m rack")
    w, d, h = 0.7, 0.25, 1.9
    p.collider((2 * w, 2 * d, h), (0, 0, h / 2))
    # The frame: uprights, feet, a top rail, the pegboard behind.
    for x in (-w + 0.03, w - 0.03):
        p.span((x - 0.03, -d, 0), (x + 0.03, d, 0.05), "Hull dark")
        p.span((x - 0.025, 0.15, 0.05), (x + 0.025, 0.2, h), "Hull dark")
    p.span((-w, 0.15, h - 0.05), (w, 0.2, h), "Hull dark")
    p.span((-w + 0.055, 0.2, 0.1), (w - 0.055, 0.22, h - 0.05), "Repaint teal")
    with p.painted():
        for j in range(16):
            for i in range(12):
                p.cyl(0.006, 0.01, (-w + 0.12 + i * 0.105, 0.197, 0.18 + j * 0.105), "Hull dark", rot=(90, 0, 0), segments=5)
    # A trough at the foot for the butts, and a rail to lean them on.
    p.span((-w + 0.05, -0.05, 0.05), (w - 0.05, 0.15, 0.12), "Hull alloy")
    p.span((-w + 0.05, 0.11, 0.85), (w - 0.05, 0.15, 0.89), "Hull dark")
    # The long guns, butts down: two Earth rifles, a shotgun, a laser rifle.
    p.source("bolt_action_rifle_7_62", at=(-0.48, 0.05, 0.12), rot=(0, -90, 0), height=1.2)
    p.source("bolt_action_rifle_7_62", at=(-0.3, 0.05, 0.12), rot=(0, -90, 0), height=1.2)
    _shotgun().stand_up(p, (-0.08, 0.05, 0.12))
    _laser_rifle().stand_up(p, (0.14, 0.05, 0.12))
    # The shelf, its ammunition cans and laser pistols on hooks above.
    p.span((0.3, -0.1, 1.0), (w - 0.06, 0.15, 1.03), "Hull alloy")
    p.source("ammo_box", at=(0.43, 0.02, 1.03), rot=(0, 0, 90), length=0.26)
    p.source("ammo_box", at=(0.55, 0.0, 1.03), rot=(0, 0, 90), length=0.26)
    for x in (0.4, 0.58):
        p.cyl(0.004, 0.08, (x, 0.16, 1.62), "Steel", rot=(90, 0, 0), segments=5)
        _laser_pistol().stand_up(p, (x, 0.1, 1.38))
    # Price tags.
    with p.painted():
        for x in (-0.48, -0.3, -0.08, 0.14, 0.49):
            p.span((x - 0.03, -0.059, 0.065), (x + 0.03, -0.053, 0.1), "Paper")
    return p


PIECES = [earth_pistol, earth_rifle, shotgun, laser_pistol, laser_rifle, katana, machete, knife,
          stun_baton, armour_vest, ammo_box, laser_cell_pack, weapon_rack]
