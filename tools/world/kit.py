"""The shared parts of the world build: the palette, a piece builder that
puts all of a piece's parts in one mesh, its box colliders and ladders, and
the export of a piece to assets/world with its entry in world.json.

Pieces are made in metres, standing on the origin, in Blender's Z-up frame;
the export turns them Y-up for the game (glTF's +Y is Blender's +Z, and
glTF's +Z is Blender's -Y). raylib loads a .glb as one model and doesn't
use its nodes, so a piece is one object, and its colliders are written to
world.json rather than into the file.

It's plain bpy, so it runs in Blender (make world) and in the harness's
bpy module (scenes/world.py)."""
import json
import math
import random
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

# How round round things are: every helper's segment counts are multiplied
# by this (finish.py raises it for the detailed build).
DETAIL = 1.0


# The most triangles a piece may have, by kind: architecture is tiled by
# the hundred (Landfall lays some 1,900 pieces, and the engine draws each
# separately), so it's kept lean and gets its detail from its texture;
# props and vehicles are where the detail goes. A piece can set its own
# (Piece.budget) for the odd set piece: the drifter, the terraformer's wreck.
BUDGET = {"kit": 4000, "prop": 20000, "item": 8000, "vehicle": 60000}

# Whether pieces are being built for the finish (finish.py). If so, a
# piece's decals (see Piece.painted) and its detailed model, if it has a
# low-poly stand-in (see Piece.lowpoly), are kept apart to be baked into its
# texture; if not, decals are joined on as geometry a few millimetres proud,
# and the detailed model is the piece.
FINISHING = False


def _n(segments: int, least: int = 3) -> int:
    return max(least, round(segments * DETAIL))


# The modular grid and the decks' height, floor to floor. A deck's ladder is
# twelve rungs at the traversal clip's 0.3 m spacing.
GRID = 2.0
DECK = 3.6
SLAB = 0.3

# The palette: ship alloy repainted many times, Crew orange, red-soil
# concrete, cheap fabric, brass, sodium lamps and blue status lights (see
# docs/look-and-feel.md). Linear RGB, as Blender's materials take it.
PALETTE = {
    "Hull alloy": (0.30, 0.31, 0.32),
    "Hull dark": (0.10, 0.11, 0.12),
    "Grating": (0.16, 0.16, 0.16),
    "Repaint navy": (0.05, 0.08, 0.17),
    "Repaint teal": (0.06, 0.22, 0.22),
    "Repaint oxide": (0.36, 0.09, 0.05),
    "Repaint green": (0.17, 0.24, 0.12),
    "Repaint cream": (0.62, 0.56, 0.42),
    "Crew orange": (0.80, 0.25, 0.03),
    "Crew grey": (0.20, 0.21, 0.22),
    "Stencil white": (0.80, 0.80, 0.76),
    "Rust": (0.30, 0.10, 0.04),
    "Red concrete": (0.42, 0.17, 0.10),
    "Fabric ochre": (0.66, 0.38, 0.10),
    "Fabric red": (0.48, 0.10, 0.07),
    "Fabric teal": (0.07, 0.30, 0.32),
    "Fabric sand": (0.62, 0.50, 0.33),
    "Fabric indigo": (0.08, 0.09, 0.26),
    "Fabric olive": (0.27, 0.27, 0.09),
    "Brass": (0.62, 0.42, 0.13),
    "Sodium lamp": (1.0, 0.55, 0.12),
    "Status blue": (0.15, 0.55, 1.0),
    "Glass": (0.42, 0.52, 0.55),
    "Screen": (0.02, 0.06, 0.09),
    "Rubber": (0.03, 0.03, 0.03),
    "Charter white": (0.80, 0.80, 0.78),
    # Charter Row: company white and navy, kept clean, gardens under glass.
    "Charter navy": (0.02, 0.04, 0.12),
    "Charter stone": (0.55, 0.52, 0.47),
    "Foliage": (0.06, 0.20, 0.05),
    "Foliage light": (0.16, 0.34, 0.07),
    "Soil": (0.10, 0.05, 0.03),
    "Wood": (0.28, 0.14, 0.06),
    "Wood dark": (0.10, 0.05, 0.02),
    # The Fringe: red dust, rock, crops, sun-bleached canvas.
    "Dust red": (0.50, 0.20, 0.09),
    "Rock red": (0.30, 0.11, 0.05),
    "Rock dark": (0.12, 0.05, 0.03),
    "Crop green": (0.18, 0.30, 0.05),
    "Wheat": (0.62, 0.45, 0.15),
    "Canvas": (0.58, 0.50, 0.38),
    "Bleached": (0.70, 0.64, 0.52),
    # The Pads: hazard paint, container colours, concrete.
    "Hazard yellow": (0.85, 0.55, 0.02),
    "Concrete": (0.33, 0.31, 0.29),
    "Container blue": (0.04, 0.12, 0.30),
    "Container green": (0.08, 0.20, 0.09),
    "Container red": (0.40, 0.05, 0.03),
    # Things: paper, wax, weapons, electronics.
    "Paper": (0.80, 0.75, 0.62),
    "Paper aged": (0.58, 0.48, 0.30),
    "Wax red": (0.45, 0.02, 0.02),
    "Leather": (0.20, 0.09, 0.04),
    "Gunmetal": (0.05, 0.055, 0.06),
    "Steel": (0.50, 0.51, 0.52),
    "Laser cyan": (0.10, 0.85, 0.90),
    "Medical white": (0.85, 0.85, 0.83),
    "Medical red": (0.60, 0.03, 0.02),
    "Circuit green": (0.02, 0.18, 0.08),
    "Copper": (0.55, 0.22, 0.08),
    "Water blue": (0.05, 0.20, 0.35),
    # Corvane: clean, new and pressed. It looks wrong here on purpose.
    "Corvane white": (0.90, 0.90, 0.92),
    "Corvane blue": (0.02, 0.18, 0.55),
}

# Fabric sets for awnings and tarps: (main, stripe).
FABRICS = {
    "Market": ("Fabric ochre", "Fabric red"),
    "Sea": ("Fabric teal", "Fabric sand"),
    "Dusk": ("Fabric indigo", "Fabric ochre"),
    "Faded": ("Fabric sand", "Fabric olive"),
}

# The repaint colours a hull plate can be.
REPAINTS = ["Repaint navy", "Repaint teal", "Repaint oxide", "Repaint green", "Repaint cream", "Crew grey"]

# What's lit: unlit in the game would be nice, but raylib's glTF loader
# keeps only the base colour, so they're just bright.
GLOWING = {"Sodium lamp", "Status blue", "Laser cyan"}


@dataclass
class Style:
    """The choices the Shape Lab offers (scenes/world.py), and that make world
    builds with: tools/world/style.json."""
    repaint: float = 0.45  # the share of hull plates painted over
    fabric: str = "Market"
    seed: int = 88

    @staticmethod
    def load(path: Path) -> "Style":
        if path.exists():
            return Style(**json.loads(path.read_text()))
        return Style()

    def save(self, path: Path) -> None:
        path.write_text(json.dumps(self.__dict__, indent=2) + "\n")


# The suffix a colour's name takes on parts made with Piece.plain().
PLAIN = "/plain"


def colour(name: str) -> str:
    """The palette colour a material's name stands for (without PLAIN)."""
    return name[:-len(PLAIN)] if name.endswith(PLAIN) else name


def material(name: str) -> bpy.types.Material:
    m = bpy.data.materials.get(name)
    if m:
        return m
    rgb = PALETTE[colour(name)]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1)
    bsdf.inputs["Roughness"].default_value = 0.75
    if colour(name) in GLOWING:
        bsdf.inputs["Emission Color"].default_value = (*rgb, 1)
        bsdf.inputs["Emission Strength"].default_value = 3
    m.diffuse_color = (*rgb, 1)
    m.roughness = 0.75
    return m


# Blender (Z up) to the game's frame (Y up).
TO_GAME = Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0)))


def game_vec(v) -> list[float]:
    return [round(c, 4) + 0.0 for c in TO_GAME @ Vector(v)]


@dataclass
class Piece:
    """One kit piece or prop, built up from parts in a single bmesh."""
    name: str
    kind: str  # "kit" (architecture), "prop" (set dressing), "item" (carried) or "vehicle"
    note: str = ""
    category: str = ""  # which part of the world it's from; set by build.py from its module
    budget: int = 0  # most triangles, if not its kind's (BUDGET)
    bm: bmesh.types.BMesh = field(default_factory=bmesh.new)
    decals: bmesh.types.BMesh = field(default_factory=bmesh.new)
    proxy: bmesh.types.BMesh = field(default_factory=bmesh.new)
    mats: list[str] = field(default_factory=list)
    colliders: list[dict] = field(default_factory=list)
    sources: list[dict] = field(default_factory=list)
    _plain: bool = False
    ladders: list[dict] = field(default_factory=list)

    def _slot(self, mat: str) -> int:
        if self._plain and not mat.endswith(PLAIN):
            mat += PLAIN
        if mat not in self.mats:
            self.mats.append(mat)
        return self.mats.index(mat)

    def _take(self, geom, mat: str) -> None:
        i = self._slot(mat)
        for f in {f for v in geom if isinstance(v, bmesh.types.BMVert) for f in v.link_faces}:
            f.material_index = i

    def box(self, size, at, mat: str, rot=(0, 0, 0)) -> None:
        """A box of size (x, y, z) centred at at, turned by rot (Euler, degrees)."""
        m = Matrix.Translation(at) @ _euler(rot) @ Matrix.Diagonal((*size, 1))
        r = bmesh.ops.create_cube(self.bm, size=1, matrix=m)
        self._take(r["verts"], mat)

    def span(self, lo, hi, mat: str) -> None:
        """A box from corner lo to corner hi."""
        lo, hi = Vector(lo), Vector(hi)
        self.box(hi - lo, (lo + hi) / 2, mat)

    def cyl(self, radius, depth, at, mat: str, rot=(0, 0, 0), segments=12, radius2=None) -> None:
        """A cylinder (or a cone, with radius2) along Z, centred at at, turned by rot."""
        m = Matrix.Translation(at) @ _euler(rot)
        r = bmesh.ops.create_cone(self.bm, cap_ends=True, segments=_n(segments), radius1=radius,
                                  radius2=radius if radius2 is None else radius2, depth=depth, matrix=m)
        self._take(r["verts"], mat)
        # Round things shade smooth along their sides, but not over their caps.
        for f in {f for v in r["verts"] for f in v.link_faces}:
            f.smooth = abs((m.to_3x3().inverted() @ f.normal).z) < 0.5

    def cable(self, a, b, radius, mat: str, sag: float = 0.0, segments=8, steps=12) -> None:
        """A cable, hose, rope or chain from a to b, hanging sag metres low
        at its middle."""
        a, b = Vector(a), Vector(b)
        points = []
        for i in range(steps + 1):
            t = i / steps
            p = a.lerp(b, t)
            p.z -= sag * 4 * t * (1 - t)
            points.append(p)
        self.tube(points, radius, mat, segments)

    def cloth(self, corners, mat: str, sag: float = 0.04, ripple: float = 0.008, thickness: float = 0.008,
              cell: float = 0.06, seed: int = 0, droop=(1, 1, 1, 1)) -> None:
        """A sheet of cloth (an awning, a tarp, a banner, a curtain) held at
        its four corners (in order round it: seen from the top side, anti-
        clockwise; the cloth's thickness is on the other side), sagging sag metres between
        them and rippled; droop weights how much each edge (from corners
        0-1, 1-2, 2-3, 3-0) hangs, 0 for one held taut along a pole."""
        from mathutils import noise
        c = [Vector(p) for p in corners]
        nu = max(2, round((c[1] - c[0]).length / cell))
        nv = max(2, round((c[3] - c[0]).length / cell))
        top, bottom = [], []
        for j in range(nv + 1):
            v = j / nv
            for i in range(nu + 1):
                u = i / nu
                p = (c[0].lerp(c[1], u)).lerp(c[3].lerp(c[2], u), v)
                # Hanging most mid-edge and mid-sheet, weighted by each edge.
                d = droop
                edge = (d[0] * (1 - v) + d[2] * v) * 4 * u * (1 - u) + (d[3] * (1 - u) + d[1] * u) * 4 * v * (1 - v)
                p.z -= sag * edge / 2
                p.z += ripple * noise.noise(Vector((p.x * 3.1, p.y * 3.1, seed * 7.3)))
                top.append(p)
        # The underside a thickness behind the top along the sheet's own
        # normal (not straight down): a steep sheet, a tent's wall or a
        # banner, keeps its thickness too.
        pts = top
        top, bottom = [], []
        w = nu + 1
        for j in range(nv + 1):
            for i in range(nu + 1):
                du = pts[j * w + min(i + 1, nu)] - pts[j * w + max(i - 1, 0)]
                dv = pts[min(j + 1, nv) * w + i] - pts[max(j - 1, 0) * w + i]
                n = du.cross(dv)
                n = n.normalized() if n.length > 1e-9 else Vector((0, 0, 1))
                p = pts[j * w + i]
                top.append(self.bm.verts.new(p))
                bottom.append(self.bm.verts.new(p - n * thickness))
        faces = []
        for j in range(nv):
            for i in range(nu):
                q = (j * w + i, j * w + i + 1, (j + 1) * w + i + 1, (j + 1) * w + i)
                faces.append(self.bm.faces.new([top[k] for k in q]))
                faces.append(self.bm.faces.new([bottom[k] for k in reversed(q)]))
        ring = [i for i in range(nu)] + [nu + j * w for j in range(nv)] + \
               [nv * w + nu - i for i in range(nu)] + [(nv - j) * w for j in range(nv)]
        for a, b in zip(ring, ring[1:] + ring[:1]):
            faces.append(self.bm.faces.new((top[b], top[a], bottom[a], bottom[b])))
        self._faces(faces, mat, smooth=True)

    def source(self, asset: str, at=(0, 0, 0), rot=(0, 0, 0), height: float | None = None,
               length: float | None = None, width: float | None = None, res: str = "1k",
               recolour: str | None = None) -> None:
        """A Poly Haven model (see polyhaven.py) as one of the piece's parts:
        goods on a stall, a pot on a stove, a lamp on a desk. It's fitted by
        one of height, length (X) or width (Y) after turning by rot, and
        stood with the middle of its foot at at."""
        self.sources.append(dict(asset=asset, at=at, rot=rot, fit={"z": height, "x": length, "y": width},
                                 res=res, recolour=recolour))

    def tube(self, points, radius, mat: str, segments=10) -> None:
        """A pipe, hose or rail through points: one tube swept along them,
        mitred at each bend, capped at its ends."""
        pts = [Vector(p) for p in points]
        pts = [p for i, p in enumerate(pts) if i == 0 or (p - pts[i - 1]).length > 1e-6]
        if len(pts) < 2:
            return
        n = _n(segments)
        dirs = [(b - a).normalized() for a, b in zip(pts, pts[1:])]
        # A frame round each run, carried from run to run so it doesn't twist.
        up = Vector((0, 0, 1)) if abs(dirs[0].z) < 0.9 else Vector((1, 0, 0))
        u = dirs[0].cross(up).normalized()
        frames = []
        for i, d in enumerate(dirs):
            if i:
                u = dirs[i - 1].rotation_difference(d) @ u
                u = (u - d * u.dot(d)).normalized()
            frames.append((u, d.cross(u)))
        rings = []
        for i, p in enumerate(pts):
            k = min(i, len(dirs) - 1)
            if 0 < i < len(pts) - 1:
                k = i - 1  # the run coming in, cut by the plane bisecting the bend
                bis = dirs[i - 1] + dirs[i]
                normal = bis.normalized() if bis.length > 1e-6 else dirs[i - 1]
            else:
                normal = None
            fu, fv = frames[k]
            ring = []
            for j in range(n):
                t = math.tau * j / n
                o = (fu * math.cos(t) + fv * math.sin(t)) * radius
                if normal is not None:
                    o = o - dirs[k] * (o.dot(normal) / max(dirs[k].dot(normal), 0.2))
                ring.append(self.bm.verts.new(p + o))
            rings.append(ring)
        sides = []
        for r0, r1 in zip(rings, rings[1:]):
            for j in range(n):
                sides.append(self.bm.faces.new((r0[j], r0[(j + 1) % n], r1[(j + 1) % n], r1[j])))
        caps = [self.bm.faces.new(list(reversed(rings[0]))), self.bm.faces.new(rings[-1])]
        self._faces(sides, mat, smooth=True)
        self._faces(caps, mat, smooth=False)

    def sphere(self, radius, at, mat: str, scale=(1, 1, 1), rot=(0, 0, 0), segments=12, rings=8) -> None:
        """A ball (or, scaled, an egg or a dome's worth of one), smooth."""
        m = Matrix.Translation(at) @ _euler(rot) @ Matrix.Diagonal((*scale, 1))
        r = bmesh.ops.create_uvsphere(self.bm, u_segments=_n(segments), v_segments=_n(rings), radius=radius, matrix=m)
        self._take(r["verts"], mat)
        for f in {f for v in r["verts"] for f in v.link_faces}:
            f.smooth = True

    def torus(self, major, minor, at, mat: str, rot=(0, 0, 0), segments=16, sides=8, arc=360.0) -> None:
        """A ring round Z (or part of one, arc degrees from +X), smooth."""
        m = Matrix.Translation(at) @ _euler(rot)
        segments, sides = _n(segments), _n(sides)
        closed = arc >= 360
        n = segments if closed else segments + 1
        rings = []
        for i in range(n):
            a = math.radians(arc) * i / segments
            c = Vector((math.cos(a) * major, math.sin(a) * major, 0))
            out = Vector((math.cos(a), math.sin(a), 0))
            rings.append([self.bm.verts.new(m @ (c + out * math.cos(b) * minor + Vector((0, 0, math.sin(b) * minor))))
                          for b in (math.tau * j / sides for j in range(sides))])
        faces = []
        for i in range(n if closed else n - 1):
            a, b = rings[i], rings[(i + 1) % n]
            for j in range(sides):
                faces.append(self.bm.faces.new((a[j], b[j], b[(j + 1) % sides], a[(j + 1) % sides])))
        if not closed:
            faces.append(self.bm.faces.new(list(reversed(rings[0]))))
            faces.append(self.bm.faces.new(rings[-1]))
        self._faces(faces, mat, smooth=True)

    def lathe(self, profile, at, mat: str, rot=(0, 0, 0), segments=16, smooth=True) -> None:
        """A turned shape round Z: profile is (radius, z) from the bottom up;
        a radius of 0 closes it to a point, and open ends are capped. Bottles,
        tanks, domes, shells, lamp shades."""
        m = Matrix.Translation(at) @ _euler(rot)
        segments = _n(segments)
        rings = []
        for r, z in profile:
            if r <= 1e-6:
                rings.append([self.bm.verts.new(m @ Vector((0, 0, z)))])
            else:
                rings.append([self.bm.verts.new(m @ Vector((math.cos(a) * r, math.sin(a) * r, z)))
                              for a in (math.tau * i / segments for i in range(segments))])
        faces = []
        for a, b in zip(rings, rings[1:]):
            for i in range(segments):
                quad = [a[i % len(a)], b[i % len(b)], b[(i + 1) % len(b)], a[(i + 1) % len(a)]]
                quad = [v for k, v in enumerate(quad) if v not in quad[:k]]
                if len(quad) >= 3:
                    faces.append(self.bm.faces.new(quad))
        cap = []
        if len(rings[0]) > 1:
            cap.append(self.bm.faces.new(list(reversed(rings[0]))))
        if len(rings[-1]) > 1:
            cap.append(self.bm.faces.new(rings[-1]))
        self._faces(faces, mat, smooth=smooth)
        self._faces(cap, mat, smooth=False)

    def prism(self, outline, thickness, at, mat: str, rot=(0, 0, 0)) -> None:
        """A flat shape cut from plate: outline is (x, z) points round it,
        in order, and it's thickness deep along Y, centred on at. Blades,
        gun bodies, signs, brackets, wedges."""
        m = Matrix.Translation(at) @ _euler(rot)
        t = thickness / 2
        front = [self.bm.verts.new(m @ Vector((x, -t, z))) for x, z in outline]
        back = [self.bm.verts.new(m @ Vector((x, t, z))) for x, z in outline]
        n = len(outline)
        faces = [self.bm.faces.new(front), self.bm.faces.new(list(reversed(back)))]
        for i in range(n):
            j = (i + 1) % n
            faces.append(self.bm.faces.new((front[i], back[i], back[j], front[j])))
        self._faces(faces, mat, smooth=False)

    def _faces(self, faces, mat: str, smooth: bool) -> None:
        i = self._slot(mat)
        for f in faces:
            f.material_index = i
            f.smooth = smooth

    @contextmanager
    def painted(self):
        """What's built inside is paint, not parts: stencils, hazard stripes,
        labels, stains. It's baked into the piece's texture (finish.py), so
        it's never a layer stacked on a surface to flicker against it; built
        without the finish, it's joined on as geometry. Keep it 4–10 mm
        proud of the surface it's painted on.

            with p.painted():
                p.span((-0.5, -0.456, 0.2), (0.5, -0.45, 0.3), "Hazard yellow")
        """
        kept = self.bm
        self.bm = self.decals
        try:
            yield
        finally:
            self.bm = kept

    @contextmanager
    def plain(self):
        """What's built inside is left exactly as built by the finish: not
        bevelled, not roughened. For thin things whose every triangle counts,
        where a bevel would multiply them: leaves, stalks, blades of grass,
        wire, mesh. (Still baked, so still shaded and dusted.)"""
        kept = self._plain
        self._plain = True
        try:
            yield
        finally:
            self._plain = kept

    @contextmanager
    def lowpoly(self):
        """What's built inside is the piece's low-poly stand-in: the few
        boxes (or prisms) the game actually draws. The piece's detailed model
        (everything built outside it) is baked onto it (finish.py): grating,
        plates, rivets and their shadows, as texture. For pieces tiled by
        the hundred (floors, walls, fences) and big plain masses (containers):
        tens of triangles where the detail is thousands. Keep its outline
        close to the detailed model's, within a few centimetres.

            with p.lowpoly():
                p.span((-1, -1, -0.3), (1, 1, 0), "Hull dark")
        """
        kept = self.bm
        self.bm = self.proxy
        try:
            yield
        finally:
            self.bm = kept

    def lettering(self, text: str, at, height: float, mat: str, facing: str = "-Y", depth: float = 0.004,
                  align: str = "CENTER", bold: float = 0.0, spacing: float = 1.0) -> None:
        """Words in Blender's own font on a face: at is the middle of the
        line's baseline-to-cap block (or its left or right end, by align),
        height its capital height, facing which way the face looks, depth
        how far the letters stand out (both ways from at). Raised letters
        on a sign are geometry; inside painted() they're paint."""
        cu = bpy.data.curves.new("lettering", "FONT")
        cu.body = text
        cu.size = height / 0.72  # the font's capitals are about 0.72 of its size
        cu.align_x = {"CENTER": "CENTER", "LEFT": "LEFT", "RIGHT": "RIGHT"}[align]
        cu.align_y = "CENTER"
        cu.extrude = depth / 2
        cu.offset = bold * height
        cu.space_character = spacing
        cu.resolution_u = 4
        o = bpy.data.objects.new("lettering", cu)
        bpy.context.scene.collection.objects.link(o)
        dg = bpy.context.evaluated_depsgraph_get()
        mesh = bpy.data.meshes.new_from_object(o.evaluated_get(dg))
        bpy.data.objects.remove(o)
        bpy.data.curves.remove(cu)
        # Flat in XY reading along +X: stood up to face -Y, then turned to facing.
        turn = {"-Y": 0, "+X": 90, "+Y": 180, "-X": -90}[facing]
        mesh.transform(Matrix.Translation(Vector(at)) @ Matrix.Rotation(math.radians(turn), 4, "Z")
                       @ Matrix.Rotation(math.radians(90), 4, "X"))
        before = len(self.bm.faces)
        self.bm.from_mesh(mesh)
        bpy.data.meshes.remove(mesh)
        self.bm.faces.ensure_lookup_table()
        i = self._slot(mat)
        for f in self.bm.faces[before:]:
            f.material_index = i
            f.smooth = False

    def stencil(self, text: str, at, height: float, mat: str, facing: str = "-Y") -> None:
        """Stencilled digits, seven-segment style, painted on a face (see
        painted): at is the middle of the text, 5 mm proud of the face, and
        facing which way the face looks."""
        with self.painted():
            self._stencil(text, at, height, mat, facing)

    def _stencil(self, text: str, at, height: float, mat: str, facing: str) -> None:
        w, t, depth = height * 0.5, height * 0.14, 0.004
        step = w + height * 0.25
        x0 = -step * (len(text) - 1) / 2
        for i, ch in enumerate(text):
            for seg in _SEGMENTS.get(ch, ""):
                (u, v), horizontal = _SEG_AT[seg]
                size = (w - t, depth, t) if horizontal else (t, depth, height / 2 - t)
                local = Vector((x0 + i * step + u * (w - t) / 2, 0, v * (height - t) / 2))
                self.box(*_face(size, local, at, facing), mat)

    def collider(self, size, at, rot=(0, 0, 0)) -> None:
        """A box the game collides with: what traversal climbs and vaults."""
        q = _euler(rot).to_quaternion()
        g = (TO_GAME.to_4x4() @ q.to_matrix().to_4x4() @ TO_GAME.to_4x4().inverted()).to_quaternion()
        self.colliders.append({
            "center": game_vec(at),
            "size": [round(abs(c), 4) for c in (size[0], size[2], size[1])],
            "rotation": [round(c, 5) + 0.0 for c in (g.x, g.y, g.z, g.w)],
        })

    def solid(self, lo, hi, mat: str) -> None:
        """A box from lo to hi that's also a collider."""
        lo, hi = Vector(lo), Vector(hi)
        self.span(lo, hi, mat)
        self.collider(hi - lo, (lo + hi) / 2)

    def ladder(self, bottom, top, facing, bottom_exit, top_exit, width=0.64) -> None:
        self.ladders.append({
            "bottom": game_vec(bottom), "top": game_vec(top), "facing": game_vec(facing),
            "bottomExit": game_vec(bottom_exit), "topExit": game_vec(top_exit), "width": width,
        })

    def turn(self, turns: int) -> None:
        """Turns everything built so far, colliders and ladders too, by
        quarter turns anticlockwise seen from above: for a piece that's
        easier to make lying along X but has to face -Y like the rest."""
        a = turns * math.pi / 2
        for bm in (self.bm, self.decals, self.proxy):
            bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(a, 3, "Z"))
        # Blender's turn about Z is the game's about Y, by the same angle.
        q = Quaternion((0, 1, 0), a)
        for c in self.colliders:
            c["center"] = [round(v, 4) + 0.0 for v in q @ Vector(c["center"])]
            x, y, z, w = c["rotation"]
            r = q @ Quaternion((w, x, y, z))
            c["rotation"] = [round(v, 5) + 0.0 for v in (r.x, r.y, r.z, r.w)]
        for ladder in self.ladders:
            for k in ("bottom", "top", "facing", "bottomExit", "topExit"):
                ladder[k] = [round(v, 4) + 0.0 for v in q @ Vector(ladder[k])]

    def build(self, collection: bpy.types.Collection | None = None) -> bpy.types.Object:
        """The piece as one object, with chamfered edges whose normals keep
        the faces flat: the outline pass pushes vertices out along their
        normals, and shared normals at the edges keep it from tearing."""
        mesh = bpy.data.meshes.new(self.name)
        if self.kind == "item" and self.bm.verts:
            # What's carried rests on whatever it's put down on: its lowest
            # point on z = 0, whatever a tilt or a slump did to it.
            low = min(v.co.z for v in self.bm.verts)
            for bm in (self.bm, self.decals, self.proxy):
                bmesh.ops.translate(bm, verts=bm.verts, vec=(0, 0, -low))
        decals = None
        if self.decals.faces:
            if FINISHING:
                decals = bpy.data.meshes.new(self.name + " decals")
                self.decals.to_mesh(decals)
                for m in self.mats:
                    decals.materials.append(material(m))
            else:
                tmp = bpy.data.meshes.new("decals")
                self.decals.to_mesh(tmp)
                self.bm.from_mesh(tmp)
                bpy.data.meshes.remove(tmp)
        self.decals.free()
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for m in self.mats:
            mesh.materials.append(material(m))
        obj = bpy.data.objects.new(self.name, mesh)
        if decals:
            d = bpy.data.objects.new(self.name + " decals", decals)
            bpy.context.scene.collection.objects.link(d)
            obj["decals"] = d.name
        (collection or bpy.context.scene.collection).objects.link(obj)
        bevel = obj.modifiers.new("Chamfer", "BEVEL")
        bevel.width = 0.008
        bevel.segments = 1
        bevel.limit_method = "ANGLE"
        bevel.angle_limit = math.radians(40)
        bevel.harden_normals = True
        if self.sources:
            self._join_sources(obj)
        if self.proxy.faces and FINISHING:
            # The game gets the stand-in; the detailed model is kept beside
            # it to bake from.
            low = bpy.data.meshes.new(self.name + " low")
            bmesh.ops.recalc_face_normals(self.proxy, faces=self.proxy.faces)
            self.proxy.to_mesh(low)
            for f in low.polygons:
                f.use_smooth = False
            for m in self.mats:
                low.materials.append(material(m))
            obj.name = self.name + " detail"
            stand = bpy.data.objects.new(self.name, low)
            (collection or bpy.context.scene.collection).objects.link(stand)
            stand["detail"] = obj.name
            stand["lowpoly"] = True
            if obj.get("decals"):
                stand["decals"] = obj["decals"]
                del obj["decals"]
            for key in ("source", "budget"):
                if key in obj:
                    stand[key] = obj[key]
            obj = stand
        self.proxy.free()
        obj["kind"] = self.kind
        obj["category"] = self.category
        if self.note:
            obj["note"] = self.note
        return obj

    def _join_sources(self, obj: bpy.types.Object) -> None:
        """The piece's sourced parts (see source) joined into obj, after its
        own chamfer is applied (so the models aren't chamfered too)."""
        import polyhaven
        dg = bpy.context.evaluated_depsgraph_get()
        mesh = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
        obj.modifiers.clear()
        obj.data = mesh
        if obj.name not in bpy.context.scene.collection.objects:
            bpy.context.scene.collection.objects.link(obj)
        parts = []
        for s in self.sources:
            o = polyhaven.load(s["asset"], s["res"])
            polyhaven.fit(o.data, s["fit"], rot=s["rot"], at=s["at"])
            if s["recolour"]:
                for m in o.data.materials:
                    m["recolour"] = s["recolour"]
            parts.append(o)
        bpy.context.view_layer.update()
        for o in bpy.context.view_layer.objects:
            o.select_set(o == obj or o in parts)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.join()
        obj["source"] = ", ".join(s["asset"] for s in self.sources)
        obj["budget"] = self.budget or BUDGET[self.kind]

    def entry(self, model: str) -> dict:
        e = {"model": model, "kind": self.kind, "category": self.category,
             "budget": self.budget or BUDGET[self.kind], "colliders": self.colliders}
        if self.ladders:
            e["ladders"] = self.ladders
        return e


# Seven segments: where each sits in a digit (in halves of its width and
# height from the middle), and whether it lies across.
_SEG_AT = {
    "a": ((0, 1), True), "g": ((0, 0), True), "d": ((0, -1), True),
    "b": ((1, 0.5), False), "c": ((1, -0.5), False),
    "e": ((-1, -0.5), False), "f": ((-1, 0.5), False),
}
_SEGMENTS = {
    "0": "abcdef", "1": "bc", "2": "abged", "3": "abgcd", "4": "fgbc",
    "5": "afgcd", "6": "afgedc", "7": "abc", "8": "abcdefg", "9": "abcdfg", "-": "g",
}


def _face(size, local, at, facing):
    """A stencil segment laid on a face looking along facing: size and
    position in the face's own frame (x across, z up) turned into the piece's."""
    at = Vector(at)
    if facing == "-Y":
        return size, at + local
    if facing == "+Y":
        return size, at + Vector((-local.x, 0, local.z))
    if facing == "-X":
        return (size[1], size[0], size[2]), at + Vector((0, -local.x, local.z))
    if facing == "+X":
        return (size[1], size[0], size[2]), at + Vector((0, local.x, local.z))
    raise ValueError(facing)


def _euler(rot) -> Matrix:
    from mathutils import Euler
    return Euler([math.radians(a) for a in rot]).to_matrix().to_4x4()


def export(obj: bpy.types.Object, path: Path) -> None:
    """The piece alone, as a .glb at path: modifiers applied, Y up, its
    materials' base colours, and nothing else."""
    path.parent.mkdir(parents=True, exist_ok=True)
    bpy.context.view_layer.update()
    for o in bpy.context.view_layer.objects:
        o.select_set(o == obj)
    bpy.context.view_layer.objects.active = obj
    kept = obj.matrix_world.copy()
    obj.matrix_world = Matrix.Identity(4)
    try:
        bpy.ops.export_scene.gltf(
            filepath=str(path), export_format="GLB", use_selection=True, export_apply=True,
            export_yup=True, export_cameras=False, export_lights=False, export_animations=False,
            export_extras=False, export_texcoords=True, export_normals=True,
            export_vertex_color="NONE", export_materials="EXPORT",
            export_image_format="JPEG", export_jpeg_quality=88)
    finally:
        obj.matrix_world = kept


def rng(style: Style, name: str) -> random.Random:
    """The same random choices for a piece on every build."""
    return random.Random(f"{style.seed}:{name}")
