"""The Second Light: the grounded colony ship Landfall is built round, seen
from outside. The Hull district is its decks cut open into streets
(docs/settlement.md); these are the pieces that make that box of streets
read as a ship: the curved skin above the walls, the prow and the engine
end that close it, the ribs over the streets, gantries, torch-cut deck
edges and vents. Ship alloy, scratched, welded and repainted for 88 years
(docs/look-and-feel.md).

How they fit the Hull in Landfall (Blender frame): the Hull is a walled,
roofless box, x -40..8, y -20..12, its walls two decks (7.2 m) high on the
2 m grid, centred on their lines and 0.3 m thick.

- hull_shell tiles along the long walls, 8 m a piece, origin on the
  wall's centre line at its top (z 7.2 in Landfall), outside to -Y: six a
  side (turned twice for the north side).
- hull_bow and hull_stern close the 32 m ends, origin at ground level on
  the end wall's centre line at its middle, outside to -Y. Their sides
  carry the shell's curve, so they meet the long sides' shells at the box's
  corners. hull_shell_corner rounds a corner where there's no bow or stern.
- hull_rib_arch spans the 32 m width over the streets, origin midway
  between the walls at wall-top height, spanning along its X: place it on
  a frame line (2 m either side of a hull_shell's centre), every 8 m.
- hull_cut_edge trims wall tops with no shell on them, 8 m a piece, origin
  on the wall's centre line at its top.
- hull_gantry and hull_vent dress the outside.

Everything here is seen from a distance, so the game draws lean stand-ins
(see kit.Piece.lowpoly) with the plating baked onto them, and nothing above
head height has a collider."""
import math

from mathutils import Euler, Matrix, Vector

from fringe import _drift
from kit import PALETTE, REPAINTS, Piece, Style, rng

CATEGORY = "The Hull"

PALETTE.setdefault("Scorch", (0.035, 0.03, 0.028))
PALETTE.setdefault("Slag", (0.22, 0.17, 0.12))
PALETTE.setdefault("Heat blue", (0.10, 0.12, 0.22))

WALL = 7.2     # the Hull's walls, two decks
HALF = 16.0    # half the box's width, wall centre to wall centre
SKIN = 0.2     # the shell plating's thickness
TOP = 10.2     # the shell's torch-cut top above the wall top, where pieces meet


# The skin's curve -----------------------------------------------------------

def bulge(z: float) -> float:
    """How far the hull's side stands out from its wall's outer face, z above
    the wall top: out to 2.6 m at 6 m, then tumbling home a little."""
    if z <= 0:
        return 0.0
    return 2.6 * math.sin(min(z, 6.0) / 6.0 * math.pi / 2) - 0.09 * max(0.0, z - 6.0) ** 1.5


def outer(z: float) -> float:
    """The skin's outer face, as y in a long wall's frame (outside -Y)."""
    return -0.15 - bulge(z)


def slope(z: float) -> float:
    """d(outer)/dz."""
    e = 0.01
    return (outer(z + e) - outer(z - e)) / (2 * e)


def _jag(r, n: int, lo: float, hi: float, end: float) -> list[float]:
    """A torch-cut line: n + 1 heights, the ends at end so neighbours meet.
    Ragged, drifting up and down as the cutter's hand did, with a few V
    bites where the crews went down to the deck below."""
    h, z = [], (lo + hi) / 2
    for _ in range(n + 1):
        z = min(hi, max(lo, z + r.uniform(-0.35, 0.35)))
        h.append(z + r.uniform(-0.08, 0.08))
    for _ in range(max(1, n // 10)):
        k = r.randrange(2, n - 1)
        depth = r.uniform(0.6, 1.3)
        h[k] -= depth
        h[k - 1] -= depth * 0.45
        h[k + 1] -= depth * 0.45
    # Easing into the fixed ends.
    for k in (0, n):
        h[k] = end
    for k in (1, n - 1):
        h[k] = (h[k] + end) / 2
    return h


def _at(h: list[float], u: float) -> float:
    """h (the jag) at u in 0..1, linearly between its points."""
    x = u * (len(h) - 1)
    i = min(int(x), len(h) - 2)
    return h[i] + (h[i + 1] - h[i]) * (x - i)


# Building blocks ------------------------------------------------------------

def _sheet(p: Piece, grid, mat: str, thickness: float, flip: bool = False) -> None:
    """A sheet through grid (rows of points, row j going up, along each row
    i): its face out along the grid's normal (flipped by flip), its back
    thickness behind, its edges closed."""
    nv, nu = len(grid), len(grid[0])
    front, back = [], []
    for j in range(nv):
        for i in range(nu):
            du = grid[j][min(i + 1, nu - 1)] - grid[j][max(i - 1, 0)]
            dv = grid[min(j + 1, nv - 1)][i] - grid[max(j - 1, 0)][i]
            n = du.cross(dv)
            n = (n.normalized() if n.length > 1e-9 else Vector((0, 0, 1))) * (-1 if flip else 1)
            front.append(p.bm.verts.new(grid[j][i]))
            back.append(p.bm.verts.new(grid[j][i] - n * thickness))
    faces = []
    for j in range(nv - 1):
        for i in range(nu - 1):
            q = (j * nu + i, j * nu + i + 1, (j + 1) * nu + i + 1, (j + 1) * nu + i)
            if flip:
                q = tuple(reversed(q))
            faces.append(p.bm.faces.new([front[k] for k in q]))
            faces.append(p.bm.faces.new([back[k] for k in reversed(q)]))
    ring = list(range(nu)) + [(j + 1) * nu - 1 for j in range(1, nv)] + \
        [nv * nu - 1 - i for i in range(1, nu)] + [(nv - 1 - j) * nu for j in range(1, nv - 1)]
    if flip:
        ring = list(reversed(ring))
    for a, b in zip(ring, ring[1:] + ring[:1]):
        faces.append(p.bm.faces.new((front[b], front[a], back[a], back[b])))
    p._faces(faces, mat, smooth=True)


def _euler_of(tx: Vector, ty: Vector, tz: Vector):
    """kit's rot (Euler degrees) for a box whose local X, Y, Z lie along tx, ty, tz."""
    m = Matrix((tx, ty, tz)).transposed()
    return tuple(math.degrees(a) for a in m.to_euler())


def _plate(p: Piece, at: Vector, tx: Vector, n: Vector, tz: Vector, w: float, h: float, mat: str,
           t: float = 0.03) -> None:
    """A plate w × h lying on a surface at at (n out of it), t thick, its back
    on the surface."""
    p.box((w, t, h), at + n * (t / 2), mat, rot=_euler_of(tx, -n, tz))


def _rivets(p: Piece, a: Vector, b: Vector, n: Vector, pitch: float, r: float = 0.022) -> None:
    """Rivet heads from a to b on a surface whose normal is n: plain, they're many."""
    steps = max(1, round((b - a).length / pitch))
    rot = tuple(math.degrees(x) for x in Vector((0, 0, 1)).rotation_difference(n).to_euler())
    with p.plain():
        for k in range(steps + 1):
            c = a.lerp(b, k / steps)
            p.cyl(r, 0.03, c, "Hull dark", rot=rot, segments=3)


def _sweep(p: Piece, path, section, mat: str, side: Vector = Vector((0, 1, 0))) -> None:
    """A beam along path (points in a plane whose normal is side), its cross
    section the closed outline section, as (across, out) pairs: across along
    side, out along the path's normal in its plane."""
    pts = [Vector(q) for q in path]
    rings = []
    for i, c in enumerate(pts):
        t = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        out = side.cross(t).normalized()
        rings.append([p.bm.verts.new(c + side * a + out * b) for a, b in section])
    m = len(section)
    faces = []
    for r0, r1 in zip(rings, rings[1:]):
        for k in range(m):
            faces.append(p.bm.faces.new((r0[k], r0[(k + 1) % m], r1[(k + 1) % m], r1[k])))
    faces.append(p.bm.faces.new(list(reversed(rings[0]))))
    faces.append(p.bm.faces.new(rings[-1]))
    p._faces(faces, mat, smooth=False)


def _ibeam(depth: float, flange: float, web: float = 0.04, t: float = 0.05):
    """An I-beam's outline, as (across, out) pairs."""
    f, d = flange / 2, depth / 2
    return [(-f, -d), (f, -d), (f, -d + t), (web / 2, -d + t), (web / 2, d - t), (f, d - t), (f, d),
            (-f, d), (-f, d - t), (-web / 2, d - t), (-web / 2, -d + t), (-f, -d + t)]


def _letters(p: Piece, text: str, place, height: float, mat: str) -> None:
    """Painted letters along a curved surface: place(s) gives the point,
    reading direction and outward normal s metres along the line from its
    middle. Each letter is set flat on the surface where it falls."""
    sizes = []
    for ch in text:
        tmp = Piece("letter", "prop")
        if ch != " ":
            tmp.lettering(ch, (0, 0, 0), height, mat, depth=0.004)
        xs = [v.co.x for v in tmp.bm.verts]
        sizes.append((tmp, (max(xs) - min(xs)) if xs else height * 0.4, (min(xs) + max(xs)) / 2 if xs else 0))
    gap = height * 0.18
    total = sum(w for _, w, _ in sizes) + gap * (len(sizes) - 1)
    s = -total / 2
    with p.painted():
        for tmp, w, mid in sizes:
            c, t, n = place(s + w / 2)
            s += w + gap
            if not tmp.bm.faces:
                tmp.bm.free()
                tmp.decals.free()
                tmp.proxy.free()
                continue
            up = n.cross(t).normalized()
            # Painted on the plates, which stand up to 4.2 cm proud of the skin.
            m = Matrix.Translation(c + n * 0.05) @ Matrix((t, -n, up)).transposed().to_4x4() @ \
                Matrix.Translation(Vector((-mid, 0, 0)))
            import bmesh
            bmesh.ops.transform(tmp.bm, matrix=m, verts=tmp.bm.verts)
            import bpy
            mesh = bpy.data.meshes.new("letter")
            tmp.bm.to_mesh(mesh)
            tmp.bm.free()
            tmp.decals.free()
            tmp.proxy.free()
            before = len(p.bm.faces)
            p.bm.from_mesh(mesh)
            bpy.data.meshes.remove(mesh)
            p.bm.faces.ensure_lookup_table()
            k = p._slot(mat)
            for f in p.bm.faces[before:]:
                f.material_index = k
                f.smooth = False


# The shell ------------------------------------------------------------------

def _shell_frame(x: float, z: float):
    """On a long side's skin: the point, along it, up it and out of it."""
    c = Vector((x, outer(z), z))
    tz = Vector((0, slope(z), 1)).normalized()
    tx = Vector((1, 0, 0))
    return c, tx, tz, tx.cross(tz).normalized()


def _skin_detail(p: Piece, r, x0: float, x1: float, top, frames) -> None:
    """The plating on a long side's skin from x0 to x1 under the jag top(x):
    plates in rows, lapped and riveted, mismatched repaints, frame lines
    outside at frames, stringers and conduit inside, scorch along the cut
    and rust run down from the rivets."""
    # Plates, each row lapping the one below.
    row_h, col_w = 1.6, 2.0
    for k in range(int(math.ceil((x1 - x0) / col_w))):
        xa = x0 + k * col_w
        xb = min(x1, xa + col_w)
        z = 0.15
        row = 0
        while True:
            zc = z + row_h / 2
            if zc > top((xa + xb) / 2) - 0.5:
                break
            c, tx, tz, n = _shell_frame((xa + xb) / 2, zc)
            mat = r.choice(REPAINTS) if r.random() < 0.25 else r.choice(["Hull alloy", "Hull alloy", "Crew grey"])
            _plate(p, c, tx, n, tz, xb - xa - 0.06, row_h - 0.04 + 0.08 * (row % 2), mat, t=0.025 + 0.01 * (row % 2))
            lo = _shell_frame(xa + 0.08, z + 0.1)[0] + n * 0.03
            hi = _shell_frame(xb - 0.08, z + 0.1)[0] + n * 0.03
            _rivets(p, lo, hi, n, 0.32)
            z += row_h
            row += 1
    # Frame lines outside: T-bars up the skin.
    for fx in frames:
        path = [Vector((fx, outer(z), z)) for z in [i * 0.5 for i in range(int(top(fx) / 0.5))]]
        if len(path) > 1:
            _sweep(p, path, [(-0.09, -0.01), (0.09, -0.01), (0.09, 0.05), (-0.09, 0.05)], "Hull dark",
                   side=Vector((1, 0, 0)))
    # Inside, toward the streets: frames, two stringers and a conduit.
    for fx in frames:
        path = [Vector((fx, outer(z) + SKIN + 0.19, z)) for z in [i * 0.5 for i in range(int(top(fx) / 0.5))]]
        if len(path) > 1:
            _sweep(p, path, _ibeam(0.35, 0.18), "Hull dark", side=Vector((1, 0, 0)))
    for zz in (2.0, 5.0):
        y = outer(zz) + SKIN + 0.12
        p.span((x0 + 0.02, y - 0.1, zz - 0.12), (x1 - 0.02, y + 0.1, zz + 0.12), "Hull alloy")
    with p.plain():
        p.tube([(x0, outer(3.4) + SKIN + 0.28, 3.4), (x1, outer(3.4) + SKIN + 0.28, 3.4)], 0.07, "Crew orange", segments=4)
    with p.painted():
        # Scorch along the cut, slag runs and rust streaks from the rivets.
        steps = int((x1 - x0) / 0.25)
        for k in range(steps):
            x = x0 + (k + 0.5) * (x1 - x0) / steps
            zt = top(x)
            c, tx, tz, n = _shell_frame(x, zt - 0.2)
            _plate(p, c + n * 0.045, tx, n, tz, (x1 - x0) / steps + 0.02, 0.32, "Scorch", t=0.004)
            if r.random() < 0.3:
                zz = r.uniform(1.0, zt - 1.0)
                c, tx, tz, n = _shell_frame(x + r.uniform(-0.2, 0.2), zz)
                _plate(p, c + n * 0.035, tx, n, tz, 0.07, r.uniform(0.6, 1.6), "Rust", t=0.004)


def _shell_grid(x0: float, x1: float, top, nu: int, nv: int):
    rows = []
    for j in range(nv + 1):
        row = []
        for i in range(nu + 1):
            x = x0 + (x1 - x0) * i / nu
            z = top(x) * j / nv
            row.append(Vector((x, outer(z), z)))
        rows.append(row)
    return rows


def hull_shell(style: Style) -> Piece:
    """8 m of the Second Light's side skin, standing on a long wall of the
    Hull and curving out and up to its torch-cut top, 9–11 m above the wall
    (17–18 m above the street): lapped plates in eighty-eight years of
    paint, rivets, frame lines, scorch where the decks were cut open.

    Origin on the wall's centre line at its top (z 7.2 in Landfall), x -4..4
    (tiles every 8 m), outside to -Y; its outer face starts flush with the
    wall's outer face (y -0.15) and stands 2.6 m out at its widest. Its ends
    are 10.2 m high, so neighbours meet. No collider: it's all overhead."""
    p = Piece("hull_shell", "kit", "8 m of ship skin on a wall top, 10.2 m high at its ends, 2.6 m out; tiles along X")
    p.budget = 600
    r = rng(style, p.name)
    jag = _jag(r, 32, 9.3, 10.8, TOP)
    top = lambda x: _at(jag, (x + 4) / 8)  # noqa: E731
    _sheet(p, _shell_grid(-4, 4, top, 32, 24), "Hull alloy", SKIN)
    _skin_detail(p, r, -4, 4, top, (-2.0, 2.0))
    with p.lowpoly():
        _sheet(p, _shell_grid(-4, 4, top, 16, 7), "Hull alloy", SKIN)
    return p


def hull_shell_corner(style: Style) -> Piece:
    """The skin turned round an outside corner of the Hull, where no prow or
    engine end does it: the shell's curve swept a quarter turn, from facing
    -Y to facing -X.

    Origin at the corner (where the two walls' centre lines cross) at the
    wall top; it fills the quarter outside the corner (x < 0, y < 0), meeting
    a hull_shell ending at x 0 along the -Y side and one (turned a quarter)
    ending at y 0 along the -X side. 10.2 m high at its ends."""
    p = Piece("hull_shell_corner", "kit", "the shell round an outside corner, origin at the corner")
    p.budget = 800
    r = rng(style, p.name)
    jag = _jag(r, 12, 9.6, 10.6, TOP)

    def grid(nu, nv):
        rows = []
        for j in range(nv + 1):
            row = []
            for i in range(nu + 1):
                u = i / nu
                a = -math.pi / 2 - u * math.pi / 2  # from pointing -Y round to -X
                z = _at(jag, u) * j / nv
                rad = -outer(z)
                row.append(Vector((math.cos(a) * rad, math.sin(a) * rad, z)))
            rows.append(row)
        return rows

    _sheet(p, grid(16, 24), "Hull alloy", SKIN, flip=True)
    with p.painted():
        for k in range(6):
            a = -math.pi / 2 - (k + 0.5) / 6 * math.pi / 2
            z = _at(jag, (k + 0.5) / 6) - 0.35
            rad = -outer(z) + 0.03
            c = Vector((math.cos(a) * rad, math.sin(a) * rad, z))
            n = Vector((math.cos(a), math.sin(a), 0))
            _plate(p, c, Vector((-math.sin(a), math.cos(a), 0)), n, Vector((0, 0, 1)), 0.5, 0.6, "Scorch", t=0.004)
    for k in range(4):
        a = -math.pi / 2 - (k + 0.5) / 4 * math.pi / 2
        n = Vector((math.cos(a), math.sin(a), 0))
        tx = Vector((-math.sin(a), math.cos(a), 0))
        z = 0.15
        row = 0
        while z + 1.6 < _at(jag, (k + 0.5) / 4) - 0.4:
            zc = z + 0.8
            rad = -outer(zc)
            tz = Vector((math.cos(a) * -slope(zc), math.sin(a) * -slope(zc), 1)).normalized()
            mat = r.choice(REPAINTS) if r.random() < 0.45 else "Hull alloy"
            _plate(p, Vector((math.cos(a) * rad, math.sin(a) * rad, zc)), tx, n, tz,
                   rad * math.pi / 2 / 4 - 0.06, 1.56, mat, t=0.025 + 0.01 * (row % 2))
            z += 1.6
            row += 1
    with p.lowpoly():
        _sheet(p, grid(8, 7), "Hull alloy", SKIN, flip=True)
    return p


# The ends -------------------------------------------------------------------

def _end_radius(z: float) -> float:
    """How far either side of the end wall's middle the hull reaches, z
    above the ground: the walls' outer faces up to the wall top, then the
    shell's curve, so a prow or stern meets the long sides' shells."""
    return HALF + 0.15 + bulge(z - WALL)


def _bow_reach(z: float) -> float:
    """How far the prow's stem stands out from the end wall, z above the
    ground: raked forward as it rises, like a ship's."""
    return 8.0 + 0.42 * min(z, 20.0)


def _bow_point(a: float, z: float) -> Vector:
    """The prow's skin at a (0 at +X, through the stem at a half turn, to
    -X) and height z: full along its sides, drawn to a rounded point."""
    c = math.cos(a)
    return Vector((_end_radius(z) * c, -_bow_reach(z) * (1 - abs(c) ** 1.25) ** 0.85, z))


def hull_bow(style: Style) -> Piece:
    """The Second Light's prow, closing one 32 m end of the Hull: a blunt
    nose rising from the dust to about 20 m, its top torch-cut where the
    decks were opened, its name and registry faded on its face, sensor
    blisters and a mast on its brow.

    Origin at ground level on the end wall's centre line at its middle,
    outside to -Y: its skin stands out up to 15 m from the wall and reaches
    16.15 m either side along X (the long walls' outer faces), then follows
    the shell's curve above 7.2 m, so it meets the long sides' hull_shells
    at the box's corners. Colliders round its foot."""
    p = Piece("hull_bow", "kit", "the prow: 33 m across, 15 m out, about 20 m high")
    p.budget = 60000
    r = rng(style, p.name)
    jag = _jag(r, 16, 18.4, 20.4, WALL + TOP)
    jag[8] = 20.6

    def grid(nu, nv):
        rows = []
        for j in range(nv + 1):
            row = []
            for i in range(nu + 1):
                u = i / nu
                a = u * math.pi
                row.append(_bow_point(a, _at(jag, u) * j / nv))
            rows.append(row)
        return rows

    _sheet(p, grid(64, 40), "Hull alloy", 0.3, flip=True)

    def frame(a, z):
        c = _bow_point(a, z)
        e = 0.002
        ta = (_bow_point(a + e, z) - _bow_point(a - e, z)).normalized()
        tz = (_bow_point(a, z + 0.05) - _bow_point(a, z - 0.05)).normalized()
        return c, ta, tz, tz.cross(ta).normalized()

    # Plates in rows, riveted, in a dozen hands' paint.
    for k in range(40):
        a = (k + 0.5) / 40 * math.pi
        z = 0.4
        row = 0
        while z + 1.6 < _at(jag, a / math.pi) - 0.6:
            c, ta, tz, n = frame(a, z + 0.8)
            w = (_bow_point(a + math.pi / 80, z) - _bow_point(a - math.pi / 80, z)).length
            mat = r.choice(REPAINTS) if r.random() < 0.1 else r.choice(["Hull alloy", "Hull alloy", "Hull alloy", "Crew grey"])
            _plate(p, c, -ta, n, tz, w - 0.05, 1.56, mat, t=0.03 + 0.012 * (row % 2))
            z += 1.6
            row += 1
    # Frame lines round the prow, and the strake where the old waterline
    # would be on a sea ship: a heavy belt at the wall top.
    for z in (WALL, 12.0):
        path = [_bow_point(k / 48 * math.pi, z) for k in range(49)]
        out = [c + (c - Vector((0, 0, z))).normalized() * 0.08 for c in path]
        p.tube(out, 0.16 if z == WALL else 0.1, "Hull dark", segments=4)
    # Sensor blisters and a mast on the brow, a hawse-like port each side.
    for a, size in ((0.35 * math.pi, 1.1), (0.65 * math.pi, 1.1), (0.5 * math.pi, 1.5)):
        c, ta, tz, n = frame(a, 16.5 if size < 1.4 else 17.6)
        p.sphere(size, c - n * 0.2, "Charter white", scale=(1, 1, 0.75), segments=10, rings=6)
        p.torus(size * 0.98, 0.08, c - n * 0.2, "Hull dark", rot=tuple(math.degrees(x) for x in
                Vector((0, 0, 1)).rotation_difference(n).to_euler()), segments=12, sides=3)
    c, ta, tz, n = frame(0.5 * math.pi, 19.0)
    with p.plain():
        p.tube([c - n * 0.3, c - n * 0.3 + Vector((0, 0, 4.5))], 0.12, "Hull dark", segments=3)
        for h in (2.0, 3.4):
            p.tube([c - n * 0.3 + Vector((-1.2, 0, h)), c - n * 0.3 + Vector((1.2, 0, h))], 0.05, "Hull dark", segments=3)
    for a in (0.22 * math.pi, 0.78 * math.pi):
        c, ta, tz, n = frame(a, 9.5)
        p.torus(1.0, 0.18, c + n * 0.05, "Hull dark", rot=tuple(math.degrees(x) for x in
                Vector((0, 0, 1)).rotation_difference(n).to_euler()), segments=14, sides=4)
        p.cyl(0.9, 0.06, c + n * 0.01, "Scorch", rot=tuple(math.degrees(x) for x in
              Vector((0, 0, 1)).rotation_difference(n).to_euler()), segments=14)

    # The name and registry, faded, across the prow's face.
    def along(z):
        samples = [(_bow_point(k / 400 * math.pi, z)) for k in range(401)]
        lengths = [0.0]
        for q0, q1 in zip(samples, samples[1:]):
            lengths.append(lengths[-1] + (q1 - q0).length)
        mid = lengths[200]

        def place(s):
            target = mid - s  # reading toward +X, which is toward a = 0
            k = max(1, min(399, next((i for i, L in enumerate(lengths) if L >= target), 399)))
            a = k / 400 * math.pi
            c, ta, tz, n = frame(a, z)
            return c, -ta, n
        return place

    _letters(p, "SECOND LIGHT", along(13.3), 1.7, "Stencil white")
    _letters(p, "CCC 0417", along(11.1), 0.9, "Stencil white")
    with p.painted():
        # Scorch along the cut, rust from the plate seams.
        for k in range(128):
            a = (k + 0.5) / 128 * math.pi
            c, ta, tz, n = frame(a, _at(jag, a / math.pi) - 0.22)
            w = (_bow_point(a + math.pi / 256, c.z) - _bow_point(a - math.pi / 256, c.z)).length
            _plate(p, c + n * 0.05, -ta, n, tz, w + 0.03, 0.36, "Scorch", t=0.004)
            if r.random() < 0.35:
                c, ta, tz, n = frame(a, r.uniform(2.5, 15.0))
                _plate(p, c + n * 0.05, -ta, n, tz, 0.08, r.uniform(0.8, 2.4), "Rust", t=0.004)
    # Dust banked against its foot.
    def drifts(segments, rings):
        dr = rng(style, p.name + " dust")
        for a, h in ((0.12 * math.pi, 1.6), (0.32 * math.pi, 1.0), (0.5 * math.pi, 1.3), (0.68 * math.pi, 0.9),
                     (0.88 * math.pi, 1.7)):
            c = _bow_point(a, 0)
            _drift(p, (c.x * 0.97, c.y * 0.92, 0), 4.0, 2.6, h, "Dust red", dr, rings=rings, segments=segments, foot=-0.1)

    drifts(18, 6)
    with p.lowpoly():
        _sheet(p, grid(32, 14), "Hull alloy", 0.3, flip=True)
        for a, size in ((0.35 * math.pi, 1.1), (0.65 * math.pi, 1.1), (0.5 * math.pi, 1.5)):
            c, ta, tz, n = frame(a, 16.5 if size < 1.4 else 17.6)
            p.sphere(size, c - n * 0.2, "Charter white", scale=(1, 1, 0.75), segments=4, rings=3)
        c, ta, tz, n = frame(0.5 * math.pi, 19.0)
        with p.plain():
            p.tube([c - n * 0.3, c - n * 0.3 + Vector((0, 0, 4.5))], 0.12, "Hull dark", segments=3)
            for h in (2.0, 3.4):
                p.tube([c - n * 0.3 + Vector((-1.2, 0, h)), c - n * 0.3 + Vector((1.2, 0, h))], 0.05, "Hull dark", segments=3)
        drifts(9, 3)
    # Its foot: a ring of boxes round the curve, 3 m high.
    for k in range(8):
        a0, a1 = k / 8 * math.pi, (k + 1) / 8 * math.pi
        q0, q1 = _bow_point(a0, 1.5), _bow_point(a1, 1.5)
        mid = (q0 + q1) / 2
        along_ = q1 - q0
        p.collider((along_.length + 0.3, 0.6, 3.0), (mid.x, mid.y, 1.5),
                   rot=(0, 0, math.degrees(math.atan2(along_.y, along_.x))))
    return p


def _stern_point(u: float, z: float) -> Vector:
    """The stern's transom at u (-1 at -X to 1 at +X) and height z: nearly
    flat, bellied 1.5 m out at its middle."""
    rad = _end_radius(z)
    x = u * rad
    return Vector((x, -1.5 * (1 - u * u) - 0.15, z))


BELLS = ((0.0, 9.6, 4.4, 9.0), (-10.0, 3.6, 3.6, 7.5), (10.0, 3.6, 3.6, 7.5))  # x, z, mouth, length


def _bell_profile(mouth: float, length: float):
    """An engine bell from its throat at the transom to its mouth, as lathe
    (radius, along) pairs, outside then back in along the inside."""
    throat = mouth * 0.38
    out = [(throat * 1.15, 0.0)] + [(throat + (mouth - throat) * (t ** 1.8), length * t) for t in
                                     (0.15, 0.35, 0.55, 0.75, 0.9, 1.0)]
    lip = [(mouth + 0.12, length), (mouth + 0.12, length + 0.1), (mouth - 0.1, length + 0.1)]
    back = [(r - 0.12, z) for r, z in reversed(out[1:])]
    return out + lip + back + [(throat * 0.9, 0.2)]


def hull_stern(style: Style) -> Piece:
    """The Second Light's engine end, closing the Hull's other 32 m end: the
    transom and three huge engine bells, cold for 88 years, the two outboard
    ones half buried in red dust, a scaffold up the starboard bell where
    someone's still stripping it, conduits across the plates.

    Origin at ground level on the end wall's centre line at its middle,
    outside to -Y: the transom reaches 16.15 m either side along X (the long
    walls' outer faces) and follows the shell's curve above 7.2 m, so it
    meets the long sides' hull_shells at the box's corners; the bells stand
    out up to 10.6 m. Colliders for the transom's foot, the buried bells and
    the dust."""
    p = Piece("hull_stern", "kit", "the engine end: 33 m across, bells 10.6 m out, about 18 m high")
    p.budget = 60000
    r = rng(style, p.name)
    jag = _jag(r, 16, 16.8, 18.0, WALL + TOP)

    def grid(nu, nv):
        rows = []
        for j in range(nv + 1):
            row = []
            for i in range(nu + 1):
                u = -1 + 2 * i / nu
                row.append(_stern_point(u, _at(jag, i / nu) * j / nv))
            rows.append(row)
        return rows

    _sheet(p, grid(48, 36), "Hull alloy", 0.3)
    # Plates, riveted, repainted.
    for k in range(16):
        u0, u1 = -1 + k / 8, -1 + (k + 1) / 8
        z = 0.3
        row = 0
        while z + 1.8 < _at(jag, (k + 0.5) / 16) - 0.6:
            c = _stern_point((u0 + u1) / 2, z + 0.9)
            q0, q1 = _stern_point(u0, z + 0.9), _stern_point(u1, z + 0.9)
            tx = (q1 - q0).normalized()
            n = tx.cross(Vector((0, 0, 1))).normalized()
            if n.y > 0:
                n = -n
            mat = r.choice(REPAINTS) if r.random() < 0.12 else r.choice(["Hull alloy", "Hull alloy", "Hull alloy", "Crew grey"])
            _plate(p, c, tx, n, Vector((0, 0, 1)), (q1 - q0).length - 0.06, 1.76, mat, t=0.03 + 0.012 * (row % 2))
            _rivets(p, q0 + Vector((0.06, 0, -0.78)) + n * 0.04, q1 + Vector((-0.06, 0, -0.78)) + n * 0.04, n, 0.4)
            z += 1.8
            row += 1

    def bells(lean: bool):
        for x, z, mouth, length in BELLS:
            base = _stern_point(x / HALF, z)
            prof = _bell_profile(mouth, length)
            p.lathe(prof, base + Vector((0, 0.3, 0)), "Hull alloy", rot=(90, 0, 0), segments=12 if lean else 24)
            if not lean:
                # Cooling rings round it, a scorch inside, the throat's flange.
                for t in (0.3, 0.55, 0.8):
                    rad = mouth * 0.38 + (mouth - mouth * 0.38) * (t ** 1.8) + 0.08
                    p.torus(rad, 0.1, base + Vector((0, -length * t + 0.3, 0)), "Heat blue", rot=(90, 0, 0),
                            segments=24, sides=4)
                p.torus(mouth * 0.38 * 1.3, 0.22, base + Vector((0, 0.1, 0)), "Hull alloy", rot=(90, 0, 0),
                        segments=20, sides=4)

    bells(False)
    with p.painted():
        for x, z, mouth, length in BELLS:
            base = _stern_point(x / HALF, z)
            # Soot down the bells' mouths, the old heat colour on the lip.
            p.torus(mouth + 0.07, 0.05, base + Vector((0, -length + 0.25, 0)), "Heat blue", rot=(90, 0, 0),
                    segments=24, sides=4)
        for k in range(30):
            u = r.uniform(-0.95, 0.95)
            zz = r.uniform(1.0, 15.0)
            c = _stern_point(u, zz)
            q = _stern_point(min(1.0, u + 0.01), zz)
            tx = (q - c).normalized()
            n = tx.cross(Vector((0, 0, 1))).normalized()
            if n.y > 0:
                n = -n
            _plate(p, c + n * 0.05, tx, n, Vector((0, 0, 1)), 0.08, r.uniform(0.8, 2.6), "Rust", t=0.004)
        for k in range(32):
            u = -1 + (k + 0.5) / 16
            zt = _at(jag, (k + 0.5) / 32) - 0.22
            c = _stern_point(u, zt)
            q = _stern_point(u + 0.02, zt)
            tx = (q - c).normalized()
            n = tx.cross(Vector((0, 0, 1))).normalized()
            if n.y > 0:
                n = -n
            _plate(p, c + n * 0.05, tx, n, Vector((0, 0, 1)), 2 * HALF / 16 + 0.05, 0.36, "Scorch", t=0.004)
        p.lettering("0417", _stern_point(0, 15.4) + Vector((0, -0.05, 0)), 1.2, "Stencil white", depth=0.004)

    # Conduits across the transom, and junction boxes.
    with p.plain():
        for zz, rad, mat in ((14.2, 0.35, "Rust"), (14.9, 0.25, "Crew orange"), (2.2, 0.3, "Hull dark")):
            path = [_stern_point(-0.92 + 1.84 * k / 12, zz) + Vector((0, -0.1 - rad, 0)) for k in range(13)]
            p.tube(path, rad, mat, segments=4)
    for u in (-0.6, -0.2, 0.35, 0.7):
        c = _stern_point(u, 13.2)
        p.box((1.0, 0.5, 1.4), c + Vector((0, -0.4, 0)), "Crew grey")

    def scaffold(lean: bool):
        # A scaffold up the starboard bell: poles, ledgers, planks, a ladder.
        x0, x1 = 6.0, 14.0
        y0, y1 = -2.0, -9.0
        with p.plain():
            for x in (x0, x0 + 4, x1):
                for y in (y0, y1):
                    p.tube([(x, y, 0), (x, y, 9.5)], 0.05, "Steel", segments=3)
            for h in (2.5, 5.0, 7.5):
                for y in (y0, y1):
                    p.tube([(x0, y, h), (x1, y, h)], 0.04, "Steel", segments=3)
                for x in (x0, x0 + 4, x1):
                    p.tube([(x, y0, h), (x, y1, h)], 0.04, "Steel", segments=3)
            if not lean:
                for h in (2.5, 5.0, 7.5):
                    for x in (x0, x0 + 4):
                        p.tube([(x, y0, h - 2.5), (x + 4, y0, h)], 0.03, "Steel", segments=3)
        for h in (5.0, 7.5):
            p.span((x0, y1, h), (x1, y1 + 0.9, h + 0.06), "Wood")
        if not lean:
            for z in [0.3 * i for i in range(1, 25)]:
                p.box((0.45, 0.04, 0.04), (x1 + 0.3, y1 + 0.5, z), "Steel")
            for x in (x1 + 0.08, x1 + 0.52):
                p.box((0.05, 0.05, 7.6), (x, y1 + 0.5, 3.8), "Steel")

    scaffold(False)
    # Red dust banked up the transom and half over the outboard bells.
    def dust(segments, rings):
        # Long low drifts along the transom, banked up over the outboard bells.
        dr = rng(style, p.name + " dust")
        for x, y, rx, ry, h in ((-10.0, -4.5, 7.0, 6.0, 3.6), (10.0, -4.5, 7.0, 6.0, 3.6), (0.0, -3.5, 6.0, 4.0, 1.5),
                                (-15.0, -1.8, 3.5, 3.2, 2.4), (15.0, -1.8, 3.5, 3.2, 2.4)):
            _drift(p, (x, y, 0), rx, ry, h, "Dust red", dr, rings=rings, segments=segments, lumps=3, foot=-0.1)
    dust(22, 7)
    with p.lowpoly():
        _sheet(p, grid(16, 9), "Hull alloy", 0.3)
        bells(True)
        scaffold(True)
        dust(10, 3)
        for u in (-0.6, -0.2, 0.35, 0.7):
            c = _stern_point(u, 13.2)
            p.box((1.0, 0.5, 1.4), c + Vector((0, -0.4, 0)), "Crew grey")
        with p.plain():
            for zz, rad, mat in ((14.2, 0.35, "Rust"), (14.9, 0.25, "Crew orange"), (2.2, 0.3, "Hull dark")):
                path = [_stern_point(-0.92 + 1.84 * k / 6, zz) + Vector((0, -0.1 - rad, 0)) for k in range(7)]
                p.tube(path, rad, mat, segments=3)
    # The transom's foot, the dust round the bells.
    p.collider((2 * HALF + 0.3, 1.6, 3.0), (0, -0.8, 1.5))
    for x in (-10.0, 10.0):
        p.collider((7.4, 7.5, 1.8), (x, -5.2, 0.9))
    p.collider((6.0, 6.0, 1.2), (0, -4.6, 0.6))
    return p


# Overhead -------------------------------------------------------------------

def _arch_path(n: int):
    """The rib's line in its plane (X across, Z up), from one wall top up
    the inside of the skin, over the streets and down the other."""
    half = []
    # Up the inside of the skin to 8 m...
    for k in range(n + 1):
        z = 8.0 * k / n
        half.append(Vector((HALF + 0.15 + bulge(z) - SKIN - 0.35, 0, z)))
    # ...then a curve inward over the streets to the crown at 10.8 m.
    a = half[-1]
    for k in range(1, 2 * n + 1):
        t = k / (2 * n)
        x = a.x * (1 - t) ** 1.6
        z = a.z + (10.8 - a.z) * math.sin(t * math.pi / 2)
        half.append(Vector((x, 0, z)))
    # From the +X foot over the crown to the -X foot.
    return list(half) + [Vector((-q.x, 0, q.z)) for q in reversed(half[:-1])]


def hull_rib_arch(style: Style) -> Piece:
    """One of the Second Light's frames, left standing where the decks were
    cut away: a riveted I-beam rib up the inside of the skin from each wall
    top and arching over the streets, its crown 10.8 m above the walls (18 m
    above the street), gusseted where it leaves the skin, a cable slung from
    it and a lamp.

    Origin midway between the walls at wall-top height (z 7.2 in Landfall);
    it spans along X, its feet on the walls' centre lines at x ±16 (turn it
    a quarter to span the Hull's 32 m width). Place it on a frame line, 2 m
    either side of a hull_shell's centre, every 8 m. No collider."""
    p = Piece("hull_rib_arch", "kit", "a rib spanning 32 m over the streets, crown 10.8 m above the walls")
    p.budget = 3000
    r = rng(style, p.name)
    path = _arch_path(8)
    _sweep(p, path, _ibeam(0.7, 0.4, 0.05, 0.06), "Hull dark")
    # Rivet lines down both flanges' outer faces, gussets where it leaves the skin.
    for k in range(len(path) - 1):
        a, b = path[k], path[k + 1]
        t = (b - a).normalized()
        out = Vector((0, 1, 0)).cross(t).normalized()
        for side in (-1, 1):
            for across in (-0.13, 0.13):
                off = out * 0.35 * side + Vector((0, across, 0))
                _rivets(p, a + off, b + off, out * side, 0.45, r=0.025)
    for sx in (-1, 1):
        g = [(0, 0), (1.6, 0), (0, 1.6)]
        q = Vector((sx * (HALF + 0.15 + bulge(8.0) - SKIN - 0.35), 0, 8.0))
        p.prism([(x * -sx, z) for x, z in g], 0.04, q + Vector((0, 0, 0.0)), "Hull alloy")
    with p.painted():
        for k in range(10):
            q = path[r.randrange(len(path))]
            p.box((0.5, 0.008, 0.25), q + Vector((0, -0.21, 0)), r.choice(["Rust", "Crew orange", "Stencil white"]))
        crown = path[len(path) // 2]
        p.lettering("F-" + str(r.randrange(40, 90)), crown + Vector((0, -0.215, -0.05)), 0.32, "Stencil white")
    # A cable slung under it and a work lamp at the crown.
    crown = path[len(path) // 2]
    with p.plain():
        p.cable(path[4] + Vector((0, 0, -0.4)), path[-5] + Vector((0, 0, -0.4)), 0.03, "Rubber", sag=1.4,
                segments=3, steps=10)
    p.box((0.5, 0.4, 0.3), crown + Vector((0, 0, -0.6)), "Crew orange")
    p.box((0.42, 0.02, 0.22), crown + Vector((0, -0.21, -0.6)), "Sodium lamp")
    with p.lowpoly():
        lean = _arch_path(4)
        _sweep(p, lean, [(-0.2, -0.35), (0.2, -0.35), (0.2, 0.35), (-0.2, 0.35)], "Hull dark")
        with p.plain():
            p.cable(path[4] + Vector((0, 0, -0.4)), path[-5] + Vector((0, 0, -0.4)), 0.03, "Rubber", sag=1.4,
                    segments=3, steps=10)
        p.box((0.5, 0.4, 0.3), crown + Vector((0, 0, -0.6)), "Crew orange")
    return p


def hull_cut_edge(style: Style) -> Piece:
    """8 m of a deck's torch-cut edge along the top of a Hull wall with no
    shell on it: the deck plating, joists and insulation in layers, cut
    ragged, slag hanging off it, scorched.

    Origin on the wall's centre line at its top (z 7.2 in Landfall), x -4..4
    (tiles every 8 m), 0.5 m deep across the wall (y -0.25..0.25), standing
    0.25–0.9 m proud of the wall top. No collider."""
    p = Piece("hull_cut_edge", "kit", "8 m of torch-cut deck edge on a wall top; tiles along X")
    p.budget = 300
    r = rng(style, p.name)
    n = 24
    tops = [r.uniform(0.25, 0.9) for _ in range(n + 1)]
    tops[0] = tops[-1] = 0.5
    outline = [(-4.0, 0.0)] + [(-4 + 8 * k / n, tops[k]) for k in range(n + 1)] + [(4.0, 0.0)]
    # The layers: deck plate, joist web, insulation, the deck below's plate.
    layers = [(0.0, 0.08, "Hull dark"), (0.08, 0.2, "Repaint cream"), (0.2, 0.26, "Hull alloy"), (0.26, 0.9, "Rust")]
    for z0, z1, mat in layers:
        pts = []
        for x, z in outline:
            pts.append((x, max(z0, min(z, z1)) if z > z0 else z0))
        clipped = [(x, z) for x, z in pts]
        clipped = [(-4.0, z0)] + [(x, z) for x, z in clipped[1:-1]] + [(4.0, z0)]
        if max(z for _, z in clipped) - z0 > 0.01:
            p.prism(clipped, 0.5 - 0.02 * layers.index((z0, z1, mat)), (0, 0, 0), mat)
    # Joist ends and slag dribbles.
    for k in range(9):
        x = -3.6 + 0.9 * k
        p.span((x - 0.05, -0.27, 0.05), (x + 0.05, 0.27, 0.22), "Hull dark")
    with p.plain():
        for k in range(18):
            x = r.uniform(-3.8, 3.8)
            z = _at(tops, (x + 4) / 8)
            for side in (-1, 1):
                p.sphere(0.04, (x, side * 0.26, z - r.uniform(0.02, 0.12)), "Slag", scale=(1, 0.6, 1.8), segments=3, rings=2)
    with p.painted():
        for k in range(16):
            x = -4 + (k + 0.5) / 2
            z = _at(tops, (x + 4) / 8)
            for side in (-1, 1):
                p.box((0.52, 0.006, 0.3), (x, side * 0.257, max(0.15, z - 0.15)), "Scorch")
    with p.lowpoly():
        p.prism(outline, 0.5, (0, 0, 0), "Hull alloy")
    return p


# Dressing outside -----------------------------------------------------------

def hull_gantry(style: Style) -> Piece:
    """A maintenance gantry against the Second Light's side: a 4 × 4 m
    scaffold tower 18 m tall, four landings with rails, a ladder up the
    inside, braces, a boom and pulley at the top over the hull, sodium work
    lamps and a power cable down to the ground. Where the Crew go up to
    patch the skin.

    Origin at the middle of its footprint, on the ground; its back (+Y) is
    the side to stand toward the hull. The skin stands out up to 2.75 m from
    the wall's centre line above 7.2 m, so set its origin at least 4.8 m out
    from the wall's centre line. Colliders: its four legs to 3 m."""
    p = Piece("hull_gantry", "prop", "4 × 4 × 18 m scaffold tower; legs collide to 3 m")
    p.budget = 20000
    r = rng(style, p.name)
    s = 1.9
    levels = (4.5, 9.0, 13.5, 18.0)
    with p.plain():
        for x in (-s, s):
            for y in (-s, s):
                p.tube([(x, y, 0.04), (x, y, 18.6)], 0.09, "Crew orange", segments=4)
        for h in [1.5 * k for k in range(1, 13)]:
            for (a, b) in (((-s, -s), (s, -s)), ((s, -s), (s, s)), ((s, s), (-s, s)), ((-s, s), (-s, -s))):
                p.tube([(a[0], a[1], h), (b[0], b[1], h)], 0.045, "Crew orange", segments=3)
        for k in range(12):
            h0, h1 = 1.5 * k, 1.5 * (k + 1)
            for (a, b) in (((-s, -s), (s, -s)), ((s, s), (-s, s)), ((-s, s), (-s, -s))):
                if k % 2:
                    a, b = b, a
                p.tube([(a[0], a[1], h0), (b[0], b[1], h1)], 0.035, "Hull dark", segments=3)
        # Landings' rails.
        for h in levels:
            for z in (h + 0.5, h + 1.0):
                for (a, b) in (((-s, -s), (s, -s)), ((s, -s), (s, s)), ((-s, s), (-s, -s))):
                    p.tube([(a[0], a[1], z), (b[0], b[1], z)], 0.02, "Hazard yellow", segments=3)
        # The ladder up the inside, its rails and rungs.
        for x in (-0.24, 0.24):
            p.tube([(x, s - 0.4, 0.02), (x, s - 0.4, 19.0)], 0.03, "Steel", segments=3)  # grab rails above the top
        for z in [0.3 * k for k in range(1, 60)]:
            p.tube([(-0.24, s - 0.4, z), (0.24, s - 0.4, z)], 0.015, "Steel", segments=3)
        # The boom and its pulley, a load line down.
        p.tube([(0, s, 18.6), (0, s + 3.5, 19.4)], 0.08, "Crew orange", segments=4)
        p.tube([(0, s + 3.4, 19.2), (0, s + 3.4, 12.0)], 0.012, "Rubber", segments=3)
        p.cable((s, -s, 18.2), (s + 0.4, -s - 1.6, 0.05), 0.025, "Rubber", sag=1.2, segments=3, steps=10)
    for h in levels:
        # Inset from the frame (whose rails and braces end at its edges).
        p.span((-s + 0.06, -s + 0.06, h - 0.06), (s - 0.06, s - 0.06, h), "Grating")
        # A hatch through each landing for the ladder, a centimetre proud.
        p.span((-0.45, s - 0.9, h - 0.07), (0.45, s - 0.1, h + 0.012), "Hull dark")
    p.torus(0.25, 0.06, (0, s + 3.5, 19.25), "Hull dark", rot=(0, 90, 0), segments=10, sides=3)
    for x in (-s, s):
        p.box((0.45, 0.25, 0.32), (x, -s - 0.2, 18.4), "Crew orange", rot=(-25, 0, 0))
        p.box((0.38, 0.02, 0.26), (x, -s - 0.33, 18.35), "Sodium lamp", rot=(-25, 0, 0))
    # Feet: base plates and sandbags.
    for x in (-s, s):
        for y in (-s, s):
            p.span((x - 0.25, y - 0.25, 0), (x + 0.25, y + 0.25, 0.04), "Hull dark")
            for k in range(2):
                p.sphere(0.2, (x + (0.35 if x < 0 else -0.35), y + r.uniform(-0.2, 0.2), 0.12 + k * 0.16), "Canvas",
                         scale=(1.3, 0.8, 0.5), segments=6, rings=3)
            p.collider((0.3, 0.3, 3.0), (x, y, 1.5))
    with p.painted():
        for h in levels:
            p.lettering(str(int(h)), (0, -s - 0.012, h - 0.35), 0.25, "Stencil white")
            p.box((0.3, 0.006, 0.006), (0, -s - 0.012, h - 0.6), "Stencil white")
    return p


def hull_vent(style: Style) -> Piece:
    """A ship's intake on the Second Light's side, 4 m wide: a framed bank of
    louvres under a hood, a grille, the bolts that hold it, rust run down
    from it and a stencilled number. Dressing for the walls and the skin.

    Its back is at y 0, to stand on a wall's outer face (y -0.15 from the
    wall's centre line), origin at the middle of its foot; 4 × 2.6 m, 0.7 m
    deep at the hood. No collider."""
    p = Piece("hull_vent", "prop", "4 × 2.6 m intake, back at y 0; no collider")
    p.budget = 400
    r = rng(style, p.name)
    w, h = 2.0, 2.6
    # The backing, a centimetre inside the frame all round.
    p.span((-w + 0.01, -0.25, 0.01), (w - 0.01, -0.01, h - 0.01), "Hull dark")
    for x0, x1 in ((-w, -w + 0.12), (w - 0.12, w)):
        p.span((x0, -0.35, 0), (x1, 0, h), "Hull alloy")
    p.span((-w, -0.35, 0), (w, 0, 0.12), "Hull alloy")
    p.span((-w, -0.35, h - 0.12), (w, 0, h), "Hull alloy")
    for x in (-0.65, 0.65):
        p.span((x - 0.05, -0.33, 0.1), (x + 0.05, -0.05, h - 0.1), "Hull alloy")
    # Louvres, bay by bay.
    for x0, x1 in ((-w + 0.13, -0.71), (-0.59, 0.59), (0.71, w - 0.13)):
        for k in range(10):
            z = 0.25 + k * 0.23
            p.box((x1 - x0, 0.02, 0.24), ((x0 + x1) / 2, -0.2, z), r.choice(["Hull alloy", "Crew grey"]), rot=(-38, 0, 0))
    # The hood.
    p.prism([(-w - 0.15, 0), (w + 0.15, 0), (w + 0.15, -0.08), (-w - 0.15, -0.08)], 0.7, (0, -0.35, h + 0.08),
            "Repaint teal", rot=(0, 0, 0))
    p.box((2 * w + 0.3, 0.08, 0.75), (0, -0.37, h + 0.2), "Repaint teal", rot=(-62, 0, 0))
    for x in [(-w + 0.2) + (2 * w - 0.4) * k / 9 for k in range(10)]:
        for z in (0.06, h - 0.06):
            p.cyl(0.02, 0.03, (x, -0.36, z), "Steel", rot=(90, 0, 0), segments=3)
    with p.painted():
        for k in range(7):
            x = r.uniform(-w + 0.2, w - 0.2)
            p.box((0.06, 0.006, r.uniform(0.3, 0.7)), (x, -0.358, 0.45), "Rust")
        p.lettering("V-" + str(r.randrange(10, 60)), (-w + 0.6, -0.358, h - 0.3), 0.14, "Stencil white")
    with p.lowpoly():
        p.span((-w, -0.35, 0), (w, 0, h), "Hull dark")
        p.box((2 * w + 0.3, 0.08, 0.75), (0, -0.37, h + 0.2), "Repaint teal", rot=(-62, 0, 0))
    return p


PIECES = [hull_shell, hull_shell_corner, hull_bow, hull_stern, hull_rib_arch, hull_gantry, hull_cut_edge, hull_vent]
