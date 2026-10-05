"""The items: what's carried, bought, sold, filed and stolen (see
docs/economy.md and docs/contracts.md). The three monies, the paper that
decides who owns what, air and water, food, parts, off-world goods, and the
tools a repair run needs.

Each rests as it would on a counter or the ground, its origin in the middle
of where it touches, at its real size; none has a collider, since they're
picked up. They're seen at arm's length and in an inventory, so they get the
most care per triangle (see GUIDE.md): real objects are Poly Haven's models,
the game's own things are modelled, and every label, value, seal and stamp
is paint, baked into the texture."""
import math

import bmesh
import bpy
from mathutils import Matrix, Vector

from kit import _SEG_AT, _SEGMENTS, BUDGET, PALETTE, Piece, Style, _euler, rng

CATEGORY = "Items"

# Colours of their own, so the finish doesn't take them for cloth or rock
# (finish.ORGANIC goes by colour name).
PALETTE.setdefault("Bottle green", (0.02, 0.09, 0.04))
PALETTE.setdefault("Protein", (0.55, 0.47, 0.36))
PALETTE.setdefault("Foil", (0.55, 0.56, 0.58))
PALETTE.setdefault("Filter media", (0.72, 0.64, 0.44))
PALETTE.setdefault("Ribbon blue", (0.05, 0.07, 0.28))
PALETTE.setdefault("Ribbon red", (0.50, 0.04, 0.04))
PALETTE.setdefault("Tape", (0.55, 0.08, 0.05))
PALETTE.setdefault("Twine", (0.55, 0.42, 0.22))
PALETTE.setdefault("Valve red", (0.55, 0.05, 0.03))
PALETTE.setdefault("Seed", (0.32, 0.21, 0.10))
PALETTE.setdefault("Ink", (0.01, 0.01, 0.03))
PALETTE.setdefault("Wine dark", (0.10, 0.012, 0.02))

# Paint (see Piece.painted) stands off what it's painted on: baked into the
# texture it's flat, and as geometry it's clear of the surface (and of the
# paint under it) by more than the 4 mm that would flicker. Layer 1 is on the
# surface, layer 2 on top of layer 1, and so on.
LIFT = 0.004
THICK = 0.001


def lift(layer: int = 1) -> float:
    """How far paint of a layer stands off its surface."""
    return LIFT + 0.0045 * (layer - 1)


class Item(Piece):
    """A piece that's carried. Its own parts get a fine bevel (the finish's
    2.5 mm, not the kit's 8 mm chamfer, which would round a card away), and
    its Poly Haven parts (source, crop) are joined on with it, the whole
    thing set down with its lowest point on z = 0."""

    def __init__(self, name: str, note: str = "", budget: int = 0):
        super().__init__(name, "item", note)
        self.budget = budget
        self.crops: list[dict] = []

    def crop(self, asset: str, keep, at=(0, 0, 0), rot=(0, 0, 0), height=None, length=None, width=None,
             res: str = "1k", recolour: str | None = None, glass: str | None = None) -> None:
        """One object out of a Poly Haven model that has several side by
        side: keep is ((x0, x1), (y0, y1)), the part of the model's own
        frame to keep. Then fitted and stood as source does. glass, a
        palette colour, is what its see-through materials become: the game
        draws everything solid, and glass's own colour is a white."""
        self.crops.append(dict(asset=asset, keep=keep, at=at, rot=rot, res=res, recolour=recolour, glass=glass,
                               fit={"z": height, "x": length, "y": width}))

    def build(self, collection=None):
        models = self.sources or self.crops
        sources, self.sources = self.sources, []
        if models:
            # Not settled on its own: it's set down with its models, below.
            self.kind = "prop"
        try:
            obj = super().build(collection)
        finally:
            self.kind = "item"
            self.sources = sources
        obj["kind"] = "item"
        chamfer = obj.modifiers.get("Chamfer")
        if chamfer:
            obj.modifiers.remove(chamfer)
        if models:
            b = obj.modifiers.new("Bevel", "BEVEL")
            b.width, b.segments = 0.0015, 2
            b.limit_method, b.angle_limit = "ANGLE", math.radians(35)
            b.harden_normals = True
            self._join_models(obj)
        return obj

    def _join_models(self, obj: bpy.types.Object) -> None:
        import polyhaven
        dg = bpy.context.evaluated_depsgraph_get()
        mesh = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
        obj.modifiers.clear()
        obj.data = mesh
        parts = []
        for s in self.sources + self.crops:
            o = polyhaven.load(s["asset"], s["res"])
            # Some models come with shape keys, which would undo fit()'s move
            # wherever the mesh is evaluated (the finish's decimation is).
            if o.data.shape_keys:
                o.shape_key_clear()
            if "keep" in s:
                (x0, x1), (y0, y1) = s["keep"]
                bm = bmesh.new()
                bm.from_mesh(o.data)
                gone = [v for v in bm.verts if not (x0 <= v.co.x <= x1 and y0 <= v.co.y <= y1)]
                bmesh.ops.delete(bm, geom=gone, context="VERTS")
                bm.to_mesh(o.data)
                bm.free()
            polyhaven.fit(o.data, s["fit"], rot=s["rot"], at=s["at"])
            if s.get("glass"):
                from kit import material
                for i, m in enumerate(o.data.materials):
                    bsdf = m and next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
                    if bsdf and "Transmission Weight" in bsdf.inputs and bsdf.inputs["Transmission Weight"].default_value > 0.5:
                        o.data.materials[i] = material(s["glass"])
            if s["recolour"]:
                for m in o.data.materials:
                    m["recolour"] = s["recolour"]
            parts.append(o)
        bpy.context.view_layer.update()
        for o in bpy.context.view_layer.objects:
            o.select_set(o == obj or o in parts)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.join()
        # A model's primitive with no material of its own gets a dark one (the
        # finish wants a material in every slot).
        from kit import material
        for i, m in enumerate(obj.data.materials):
            if m is None:
                obj.data.materials[i] = material("Hull dark")
        # Set down: its lowest point on z = 0, its paint with it.
        low = min(v.co.z for v in obj.data.vertices)
        obj.data.transform(Matrix.Translation((0, 0, -low)))
        decals = bpy.data.objects.get(obj.get("decals", ""))
        if decals:
            decals.data.transform(Matrix.Translation((0, 0, -low)))
        obj["source"] = ", ".join(s["asset"] for s in self.sources + self.crops)
        obj["budget"] = self.budget or BUDGET["item"]

    def lettering(self, text: str, at, height: float, mat: str, layer: int = 1) -> None:
        """Digits painted on a face that looks up, reading along +X with their
        tops towards +Y: at is the middle of the text on the surface."""
        w, t = height * 0.5, height * 0.14
        step = w + height * 0.25
        x0 = -step * (len(text) - 1) / 2
        z = at[2] + lift(layer) + THICK / 2
        with self.painted():
            for i, ch in enumerate(text):
                for seg in _SEGMENTS.get(ch, ""):
                    (u, v), across = _SEG_AT[seg]
                    size = (w - t, t, THICK) if across else (t, height / 2 - t, THICK)
                    self.box(size, (at[0] + x0 + i * step + u * (w - t) / 2, at[1] + v * (height - t) / 2, z), mat)

    def paint_up(self, lo, hi, z: float, mat: str, rot: float = 0, layer: int = 1) -> None:
        """A patch of paint, lo to hi (x, y), on a face looking up at z,
        turned rot degrees about its middle."""
        with self.painted():
            c = ((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, z + lift(layer) + THICK / 2)
            self.box((hi[0] - lo[0], hi[1] - lo[1], THICK), c, mat, rot=(0, 0, rot))

    def paint_front(self, lo, hi, y: float, mat: str, facing: int = -1, layer: int = 1) -> None:
        """A patch of paint, lo to hi (x, z), on a face looking along facing
        Y (-1 the front) at y."""
        with self.painted():
            yy = y + facing * (lift(layer) + THICK / 2)
            self.box((hi[0] - lo[0], THICK, hi[1] - lo[1]), ((lo[0] + hi[0]) / 2, yy, (lo[1] + hi[1]) / 2), mat)

    def sleeve(self, radius: float, z0: float, z1: float, mat: str, at=(0, 0), segments=48, layer: int = 1) -> None:
        """A band of paint round a cylinder of radius standing at at: a
        label. Open-ended, so bands side by side don't overlap."""
        R = radius + lift(layer) + THICK / 2
        with self.painted():
            rings = [[self.bm.verts.new((at[0] + math.cos(a) * R, at[1] + math.sin(a) * R, z))
                      for a in (math.tau * k / segments for k in range(segments))] for z in (z0, z1)]
            faces = [self.bm.faces.new((rings[0][k], rings[0][(k + 1) % segments], rings[1][(k + 1) % segments], rings[1][k]))
                     for k in range(segments)]
            self._faces(faces, mat, smooth=True)

    def stencil_on(self, text: str, x: float, y: float, z: float, height: float, mat: str, layer: int = 1) -> None:
        """Stencilled digits painted on a face looking -Y at y."""
        # kit's stencil is 4 mm deep about its middle: its back is at the
        # layer's lift, its face 4 mm further out.
        self.stencil(text, (x, y - lift(layer) - 0.002, z), height, mat)

    def lid_paint(self, centre, rot_x: float, size, local, mat: str) -> None:
        """Paint on the inside (-Y) face of a lid: a box of size centred at
        centre, turned rot_x degrees about X. local is (x, z) on its face."""
        m = Matrix.Translation(centre) @ _euler((rot_x, 0, 0))
        with self.painted():
            for (x, z, w, h) in local:
                at = m @ Vector((x, -size[1] / 2 - lift(1) - THICK / 2, z))
                self.box((w, THICK, h), at, mat, rot=(rot_x, 0, 0))


# The finish bevels edges sharper than 35°: anything round needs at least 11
# sides when it's finished (5 here, DETAIL times), or every edge of it is
# bevelled. ROUND is that least.
ROUND = 5

# A hexagon's turn so one of its flats faces -Y (bmesh starts its first
# corner on +Y).
HEX = 30


def ngon(p: Piece, sides: int, radius: float, depth: float, at, mat: str, rot=(0, 0, 0), radius2=None) -> None:
    """A prism of so many sides exactly (a hex nut, a hexagonal cell): kit's
    cylinders get rounder with the finish's DETAIL; this doesn't."""
    m = Matrix.Translation(at) @ _euler(rot)
    r = bmesh.ops.create_cone(p.bm, cap_ends=True, segments=sides, radius1=radius,
                              radius2=radius if radius2 is None else radius2, depth=depth, matrix=m)
    p._take(r["verts"], mat)


def rounded(w: float, h: float, r: float, n: int = 3) -> list[tuple]:
    """The outline of a w × h rectangle with corners rounded to r, about
    its middle (for prism)."""
    pts = []
    for cx, cy, a0 in ((w / 2 - r, h / 2 - r, 0), (-w / 2 + r, h / 2 - r, 90), (-w / 2 + r, -h / 2 + r, 180), (w / 2 - r, -h / 2 + r, 270)):
        for i in range(n + 1):
            a = math.radians(a0 + 90 * i / n)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def flat(p: Piece, outline, thickness: float, at, mat: str, turn: float = 0) -> None:
    """A flat shape lying down (a card, a note, a plate): outline is (x, y)
    points round it, and it's thickness thick, its bottom at at's z."""
    # prism's outline is (x, z); laid flat by a quarter turn about X its z
    # runs along -Y, so the outline's y is negated to keep it the right way.
    p.prism([(x, -y) for x, y in outline], thickness, (at[0], at[1], at[2] + thickness / 2), mat, rot=(90, 0, turn))


# Money ---------------------------------------------------------------------

CHIT = (0.086, 0.054)


def _chit_paint(p: Item, x, y, z, turn: float, value: str) -> None:
    """A chit's face: the Exchange's navy band, a guilloche of fine lines,
    the brass foil seal, its value and serial, and a Registrar's tick."""
    c, s = math.cos(math.radians(turn)), math.sin(math.radians(turn))

    def at(u, v):
        return (x + u * c - v * s, y + u * s + v * c)

    with p.painted():
        h1, h2 = z + lift(1) + THICK / 2, z + lift(2) + THICK / 2
        p.box((CHIT[0] - 0.006, 0.011, THICK), (*at(0, 0.016), h1), "Charter navy", rot=(0, 0, turn))
        for i in range(4):
            p.box((0.062, 0.0007, THICK), (*at(0.008, -0.0175 - i * 0.0022), h1), "Ribbon blue", rot=(0, 0, turn + 2 * (i % 2)))
        p.cyl(0.0105, THICK, (*at(-0.026, -0.009), h1), "Brass", segments=16)
        p.cyl(0.0065, THICK, (*at(-0.026, -0.009), h2), "Charter navy", segments=12)
    if turn == 0:
        p.lettering(value, (x + 0.02, y - 0.001, z), 0.016, "Charter navy", layer=2)
        p.lettering("0427", (x + 0.022, y + 0.016, z), 0.006, "Paper", layer=2)


def mark_chit(style: Style) -> Item:
    """A mark chit: Exchange credit, a stamped card worth what the Exchange
    says it is. The legal money, good for anything filed."""
    p = Item("mark_chit", "8.6 × 5.4 cm card; Exchange credit")
    flat(p, rounded(*CHIT, 0.004), 0.0012, (0, 0, 0), "Paper")
    _chit_paint(p, 0, 0, 0.0012, 0, "50")
    return p


def mark_stack(style: Style) -> Item:
    """A stack of mark chits in a brass bulldog clip: a day's wages, or a
    bribe."""
    p = Item("mark_stack", "a dozen chits in a clip")
    r = rng(style, p.name)
    t, n = 0.0012, 12
    for i in range(n):
        turn = r.uniform(-7, 7) if i < n - 1 else 0
        flat(p, rounded(*CHIT, 0.004), t, (r.uniform(-0.003, 0.003), r.uniform(-0.002, 0.002), i * (t + 0.0003)),
             "Paper", turn=turn)
    top = n * (t + 0.0003)
    _chit_paint(p, 0, 0, top - 0.0003, 0, "50")
    # The clip on the stack's end: two jaws, a spring and its lever loops.
    x = -0.036
    for z, sz in ((-0.0045, 1), (top + 0.004, -1)):
        p.box((0.022, 0.03, 0.0018), (x, 0, z), "Brass", rot=(0, sz * -6, 0))
    p.span((x - 0.014, -0.015, -0.0055), (x - 0.01, 0.015, top + 0.0055), "Brass")
    p.cyl(0.0035, 0.032, (x - 0.012, 0, top / 2), "Brass", rot=(90, 0, 0), segments=10)
    for side in (-1, 1):
        p.torus(0.009, 0.0011, (x - 0.021, side * 0.011, top + 0.009), "Steel", rot=(90, 0, 0), segments=12, sides=5, arc=200)
    return p


def scrip_bundle(style: Style) -> Item:
    """A bundle of Corvane scrip: the old company's notes, worthless for
    fifty years, in a paper band. A long bet on the Receivership honouring
    them."""
    p = Item("scrip_bundle", "15 × 7 cm notes, 3 cm thick")
    r = rng(style, p.name)
    w, d = 0.15, 0.07
    # The block of notes, a few loose ones fanning at the sides, the top one askew.
    flat(p, rounded(w, d, 0.002), 0.024, (0, 0, 0), "Corvane white")
    for i, z in enumerate((0.004, 0.011, 0.018)):
        flat(p, rounded(w, d, 0.002), 0.002, (r.uniform(-0.004, 0.004), r.uniform(-0.002, 0.002), z),
             "Corvane white", turn=r.uniform(-3, 3))
    flat(p, rounded(w, d, 0.002), 0.0015, (0.003, -0.002, 0.024), "Corvane white", turn=3)
    top = 0.0255
    # The note's print: Corvane blue panels, the company's roundel, its value.
    for u, ww in ((-0.05, 0.035), (0.05, 0.035)):
        p.paint_up((u - ww / 2, -0.03), (u + ww / 2, 0.03), top, "Corvane blue", rot=3)
    with p.painted():
        p.cyl(0.016, THICK, (0.045, 0.0, top + lift(2) + THICK / 2), "Corvane white", segments=24)
        p.cyl(0.011, THICK, (0.045, 0.0, top + lift(3) + THICK / 2), "Corvane blue", segments=24)
    p.lettering("100", (-0.05, -0.002, top), 0.014, "Corvane white", layer=2)
    # Fine blue lines round the edges of the block: the notes' print.
    for z in (0.006, 0.013, 0.02):
        p.paint_front((-w / 2, z - 0.0008), (w / 2, z + 0.0008), -d / 2, "Corvane blue")
    # The band, aged paper, stamped with a lot number.
    with p.painted():
        p.span((-0.017, -d / 2 - lift(2) - THICK, -lift(2) - THICK), (0.017, d / 2 + lift(2) + THICK, top + lift(2) + THICK), "Paper aged")
    p.lettering("38", (0, 0.012, top), 0.012, "Wax red", layer=3)
    return p


def _cartridge(p: Piece, at, scale: float = 1.0, pleats: bool = True, label: str = "40", top_only: bool = False) -> None:
    """An air-filter cartridge standing at at: pleated media in a steel
    cage between rubber-sealed end caps, a brass spigot on top and the
    Crew's orange band. top_only is just what shows of one standing in a
    case's foam: its top cap, band and spigot."""
    x, y, z = at
    s = scale
    r, h = 0.068 * s, 0.31 * s
    if top_only:
        p.lathe([(0.0, h - 0.05 * s), (r + 0.006 * s, h - 0.05 * s), (r + 0.006 * s, h), (0.0, h)], (x, y, z), "Hull dark", segments=7)
        p.lathe([(0.0, 0.0), (0.028 * s, 0.0), (0.028 * s, 0.03 * s), (0.0, 0.03 * s)], (x, y, z + h), "Brass", segments=ROUND)
        if isinstance(p, Item):
            p.sleeve(r + 0.006 * s, z + h - 0.046 * s, z + h - 0.012 * s, "Crew orange", at=(x, y))
        return
    # End caps: pressed steel, rolled lips, gaskets; the top one deep enough
    # to carry the band.
    for z0, flip in ((0.0, 1), (h, -1)):
        deep = 0.03 if flip > 0 else 0.055
        prof = [(0.0, 0.0), (r + 0.004 * s, 0.0), (r + 0.006 * s, 0.006 * s), (r + 0.006 * s, (deep - 0.006) * s),
                (r + 0.004 * s, deep * s), (r - 0.004 * s, (deep + 0.002) * s)]
        if flip < 0:
            prof = [(rr, -zz) for rr, zz in reversed(prof)]
        p.lathe(prof, (x, y, z + z0), "Hull dark", segments=12)
    p.torus(r - 0.008 * s, 0.006 * s, (x, y, z + h + 0.002 * s), "Rubber", segments=12, sides=ROUND)
    # The pleats, a star of folded media, and the cage's straps over them.
    if pleats:
        n = 20
        bm = p.bm
        rings = []
        for zz in (z + 0.03 * s, z + h - 0.055 * s):
            rings.append([bm.verts.new((x + math.cos(a) * rr, y + math.sin(a) * rr, zz))
                          for k in range(2 * n)
                          for a, rr in [(math.tau * k / (2 * n), r - (0.012 * s if k % 2 else 0.001 * s))]])
        faces = [bm.faces.new((rings[0][k], rings[0][(k + 1) % (2 * n)], rings[1][(k + 1) % (2 * n)], rings[1][k]))
                 for k in range(2 * n)]
        faces += [bm.faces.new(list(reversed(rings[0]))), bm.faces.new(rings[1])]
        p._faces(faces, "Filter media", smooth=False)
        for k in range(4):
            a = math.tau * k / 4 + 0.3
            p.span((x + math.cos(a) * r - 0.004 * s, y + math.sin(a) * r - 0.004 * s, z + 0.03 * s),
                   (x + math.cos(a) * r + 0.004 * s, y + math.sin(a) * r + 0.004 * s, z + h - 0.055 * s), "Steel")
        for zz in (0.1, 0.2):
            p.torus(r, 0.003 * s, (x, y, z + zz * s), "Steel", segments=12, sides=ROUND)
    else:
        p.cyl(r - 0.002 * s, h - 0.085 * s, (x, y, z + 0.03 * s + (h - 0.085 * s) / 2), "Filter media", segments=10)
    # The spigot, its thread as ridges, and its dust cap.
    p.lathe([(0.0, 0.0), (0.03 * s, 0.0), (0.03 * s, 0.012 * s), (0.026 * s, 0.014 * s), (0.026 * s, 0.03 * s),
             (0.02 * s, 0.032 * s), (0.0, 0.032 * s)], (x, y, z + h + 0.004 * s), "Brass", segments=8)
    if isinstance(p, Item):
        for i in range(3):
            p.sleeve(0.026 * s, z + h + (0.0175 + 0.004 * i) * s, z + h + (0.0195 + 0.004 * i) * s, "Hull dark", at=(x, y))
    # The band: Crew orange round the top, its rating stencilled.
    if isinstance(p, Item):
        p.sleeve(r + 0.006 * s, z + h - 0.046 * s, z + h - 0.012 * s, "Crew orange", at=(x, y))
        if label:
            p.stencil_on(label, x, y - r - 0.006 * s, z + h - 0.029 * s, 0.022 * s, "Ink", layer=2)


def filter_cartridge(style: Style) -> Item:
    """An air-filter cartridge: what everyone breathes through, and what the
    Fringe uses for money."""
    p = Item("filter_cartridge", "0.15 m across, 0.35 m tall")
    _cartridge(p, (0, 0, 0))
    return p


def filter_case(style: Style) -> Item:
    """A carry case of six filter cartridges: the Fringe's money in bulk, and
    what a dust storm makes twice as dear. A moulded Crew case, its lid
    thrown back, the cartridges' tops standing out of the foam."""
    p = Item("filter_case", "0.52 × 0.36 m case of six, lid open")
    w, d, h = 0.26, 0.175, 0.2
    wall = 0.012
    # The tub: floor, walls with a lip, ribs moulded down the sides.
    p.span((-w, -d, 0.006), (w, d, 0.02), "Crew grey")
    for sx in (-1, 1):
        p.span((sx * w - (wall if sx > 0 else 0), -d, 0.006), (sx * w + (wall if sx < 0 else 0), d, h), "Crew grey")
    for sy in (-1, 1):
        p.span((-w, sy * d - (wall if sy > 0 else 0), 0.006), (w, sy * d + (wall if sy < 0 else 0), h), "Crew grey")
    p.span((-w - 0.006, -d - 0.006, h - 0.014), (w + 0.006, d + 0.006, h + 0.005), "Hull dark")
    for i in (0, 2, 4):
        xx = -0.18 + i * 0.09
        for sy in (-1, 1):
            p.span((xx - 0.008, sy * d - (0.006 if sy < 0 else -0.0), 0.03), (xx + 0.008, sy * d + (0.006 if sy > 0 else 0.0), h - 0.02), "Crew grey")
    # Feet, latches, and the carrying handle on its side.
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.span((sx * (w - 0.04) - 0.02, sy * (d - 0.03) - 0.015, 0.0), (sx * (w - 0.04) + 0.02, sy * (d - 0.03) + 0.015, 0.006), "Rubber")
    for x in (-0.15, 0.15):
        p.span((x - 0.02, -d - 0.018, h - 0.045), (x + 0.02, -d - 0.0061, h + 0.004), "Steel")
        p.span((x - 0.012, -d - 0.024, h - 0.03), (x + 0.012, -d - 0.018, h - 0.01), "Hull dark")
    for x in (-0.07, 0.07):
        p.span((x - 0.012, -d - 0.03, 0.09), (x + 0.012, -d - 0.006, 0.12), "Hull dark")
    p.tube([(-0.07, -d - 0.04, 0.105), (0.07, -d - 0.04, 0.105)], 0.011, "Rubber", segments=ROUND)
    # Foam, and the six cartridges' tops standing in it.
    p.span((-w + wall, -d + wall, 0.02), (w - wall, d - wall, h - 0.08), "Hull dark")
    for i in range(3):
        for j in range(2):
            _cartridge(p, ((i - 1) * 0.155, (j - 0.5) * 0.15, 0.022), scale=0.95, top_only=True)
    # The lid, thrown back on its hinges, egg-crate foam inside.
    lid = 0.06
    p.box((2 * w, lid, 2 * d), (0, d + lid / 2 + 0.004, h + d - 0.01), "Crew grey", rot=(-6, 0, 0))
    for x in (-0.16, 0.16):
        p.cyl(0.009, 0.06, (x, d + 0.006, h - 0.004), "Steel", rot=(0, 90, 0), segments=ROUND)
    p.lid_paint((0, d + lid / 2 + 0.004, h + d - 0.01), -6, (2 * w, lid, 2 * d),
                [(-0.18 + i * 0.12, -0.1 + j * 0.1, 0.08, 0.07) for i in range(4) for j in range(3)], "Hull dark")
    p.stencil_on("6", 0.135, -d, 0.07, 0.07, "Stencil white")
    p.paint_front((-0.165, 0.035), (-0.105, 0.05), -d, "Crew orange")
    return p


# Paper ---------------------------------------------------------------------

def _sheet(p: Item, w: float, h: float, mat: str, curl: float = 0.004) -> None:
    """A sheet of paper lying on a table, its corners curling up a little."""
    p.cloth([(-w / 2, -h / 2, curl), (w / 2, -h / 2, curl * 0.6), (w / 2, h / 2, curl), (-w / 2, h / 2, curl * 0.8)],
            mat, sag=curl, ripple=0.0006, thickness=0.0008, cell=0.03)


def contract_paper(style: Style) -> Item:
    """A filed contract: the terms typed out, signed, stamped by a Registrar
    and clipped. Legal means this."""
    p = Item("contract_paper", "A4 sheet, Registrar-stamped and clipped")
    w, h = 0.21, 0.297
    _sheet(p, w, h, "Paper")
    top = 0.006
    r = rng(style, p.name)
    # The Exchange's letterhead, the terms as lines, the two signatures.
    p.paint_up((-w / 2 + 0.014, h / 2 - 0.042), (w / 2 - 0.014, h / 2 - 0.02), top, "Charter navy")
    p.lettering("41", (w / 2 - 0.03, h / 2 - 0.031, top), 0.012, "Paper", layer=2)
    for i in range(13):
        y = h / 2 - 0.06 - i * 0.0145
        if i in (4, 9):
            continue
        p.paint_up((-w / 2 + 0.018, y - 0.0016), (-w / 2 + 0.018 + r.uniform(0.12, 0.172), y + 0.0016), top, "Ink")
    for x0 in (-0.085, 0.015):
        p.paint_up((x0, -h / 2 + 0.05), (x0 + 0.07, -h / 2 + 0.0515), top, "Ink")
        with p.painted():
            pts = [(x0 + 0.005 + k * 0.009, -h / 2 + 0.06 + (0.004 if k % 2 else -0.002)) for k in range(7)]
            for (ax, ay), (bx, by) in zip(pts, pts[1:]):
                L = math.hypot(bx - ax, by - ay)
                p.box((L, 0.0011, THICK), ((ax + bx) / 2, (ay + by) / 2, top + lift(1) + THICK / 2), "Ribbon blue",
                      rot=(0, 0, math.degrees(math.atan2(by - ay, bx - ax))))
    # The stamp: a red frame, askew, with the filing's number; and the seal.
    with p.painted():
        cx, cy, a = 0.045, -0.06, -12
        c, s = math.cos(math.radians(a)), math.sin(math.radians(a))
        for (u, v, sw, sh) in ((0, 0.017, 0.07, 0.003), (0, -0.017, 0.07, 0.003), (0.0335, 0, 0.003, 0.037), (-0.0335, 0, 0.003, 0.037)):
            p.box((sw, sh, THICK), (cx + u * c - v * s, cy + u * s + v * c, top + lift(2) + THICK / 2), "Wax red", rot=(0, 0, a))
        p.cyl(0.017, THICK, (-0.07, -h / 2 + 0.03, top + lift(1) + THICK / 2), "Brass", segments=24)
        p.cyl(0.011, THICK, (-0.07, -h / 2 + 0.03, top + lift(2) + THICK / 2), "Charter navy", segments=20)
    p.lettering("2207", (0.045, -0.06, top), 0.011, "Wax red", layer=2)
    # A paperclip at the top corner.
    y0, x0 = h / 2 - 0.004, -w / 2 + 0.035
    clip = [(x0, y0 + 0.01, 0.003), (x0, y0 - 0.03, 0.003), (x0 + 0.008, y0 - 0.036, 0.003), (x0 + 0.016, y0 - 0.03, 0.003),
            (x0 + 0.016, y0 + 0.012, 0.003), (x0 + 0.008, y0 + 0.018, 0.003), (x0 + 0.002, y0 + 0.012, 0.003),
            (x0 + 0.002, y0 - 0.022, 0.003)]
    p.tube(clip, 0.0009, "Steel", segments=6)
    return p


def sealed_filing(style: Style) -> Item:
    """A sealed filing: an envelope under a Registrar's wax seal, for a
    courier to carry across Landfall before the session closes."""
    p = Item("sealed_filing", "23 × 12 cm envelope, wax-sealed")
    w, h, t = 0.23, 0.12, 0.007
    flat(p, rounded(w, h, 0.004), t, (0, 0, 0), "Paper aged")
    # The flap folded over, its edges shadowed: paint (a layer of paper
    # under the wax would flicker against it).
    with p.painted():
        for (ax, ay), (bx, by) in (((-w / 2 + 0.004, h / 2 - 0.004), (0.0, -0.012)), ((w / 2 - 0.004, h / 2 - 0.004), (0.0, -0.012))):
            # Stopping short of the wax.
            bx, by = ax + (bx - ax) * 0.7, ay + (by - ay) * 0.7
            L = math.hypot(bx - ax, by - ay)
            p.box((L, 0.0016, THICK), ((ax + bx) / 2, (ay + by) / 2, t + lift(1)), "Wood",
                  rot=(0, 0, math.degrees(math.atan2(by - ay, bx - ax))))
    # Ribbon under the wax, its two tails trailing out.
    with p.painted():
        for a, ln in ((-62, 0.075), (-110, 0.07)):
            rad = math.radians(a)
            mid = 0.024 + ln / 2
            p.box((ln, 0.007, THICK), (math.cos(rad) * mid, -0.012 + math.sin(rad) * mid, t + lift(1)),
                  "Ribbon blue", rot=(0, 0, a))
    # The seal: a pooled blob of wax with drips and the pressed ring.
    sx, sy = 0.0, -0.012
    p.lathe([(0.0, 0.0), (0.021, 0.0), (0.022, 0.002), (0.019, 0.0045), (0.012, 0.0055), (0.0, 0.0055)],
            (sx, sy, t), "Wax red", segments=18)
    with p.painted():
        for a, rr in ((20, 0.006), (150, 0.005), (250, 0.0045)):
            rad = math.radians(a)
            p.cyl(rr, THICK, (sx + math.cos(rad) * 0.022, sy + math.sin(rad) * 0.022, t + lift(1)), "Wax red", segments=10)
    wax = t + 0.0055
    p.torus(0.011, 0.0016, (sx, sy, wax), "Wax red", segments=18, sides=5)
    p.lettering("41", (sx, sy, wax), 0.008, "Ink")
    # Address and courier stamps.
    r = rng(style, p.name)
    for i in range(3):
        p.paint_up((0.03, -0.04 + i * 0.008), (0.03 + r.uniform(0.04, 0.07), -0.037 + i * 0.008), t, "Ink")
    p.paint_up((-0.1, -0.05), (-0.07, -0.035), t, "Wax red", rot=8)
    return p


def ledger_book(style: Style) -> Item:
    """A ledger: a Charter Family office's book of who owes what. Lifting one
    is a theft contract."""
    p = Item("ledger_book", "24 × 32 cm, 5.5 cm thick, strapped")
    w, h, t, c = 0.24, 0.32, 0.055, 0.0045
    # Covers overhanging a page block; the page edges' lines are paint.
    flat(p, rounded(w, h, 0.006), c, (0.004, 0, 0), "Leather")
    flat(p, rounded(w, h, 0.006), c, (0.004, 0, t - c), "Leather")
    p.span((-w / 2 + 0.012, -h / 2 + 0.006, c), (w / 2 - 0.002, h / 2 - 0.006, t - c), "Paper aged")
    for k in range(5):
        z = c + 0.006 + k * 0.009
        for x0, x1 in ((-0.08, 0.036), (0.069, w / 2 - 0.004)):
            p.paint_front((x0, z - 0.0006), (x1, z + 0.0006), -h / 2 + 0.006, "Paper")
        with p.painted():
            p.box((THICK, h - 0.014, 0.0012), (w / 2 - 0.002 + lift(1) + THICK / 2, 0, z), "Paper")
    # The spine: rounded, banded.
    p.lathe([(0.0, -h / 2), (t / 2, -h / 2), (t / 2 + 0.002, -h / 2 + 0.003), (t / 2 + 0.002, h / 2 - 0.003), (t / 2, h / 2), (0.0, h / 2)],
            (-w / 2 + 0.008, 0, t / 2), "Leather", rot=(-90, 0, 0), segments=16)
    for y in (-0.11, -0.04, 0.04, 0.11):
        p.torus(t / 2 + 0.0025, 0.0028, (-w / 2 + 0.008, y, t / 2), "Leather", rot=(90, 0, 0), segments=16, sides=5)
    # Brass corner pieces on the open side, folded over both covers.
    for sy in (-1, 1):
        cy = sy * (h / 2 - 0.004)
        with p.painted():
            flat(p, [(w / 2 + 0.004, cy), (w / 2 - 0.03, cy), (w / 2 + 0.004, cy - sy * 0.03)], THICK, (0, 0, t + lift(1)), "Brass")
            p.box((THICK, 0.03, t), (w / 2 + 0.004 + lift(1), cy - sy * 0.015, t / 2), "Brass")
    # A strap across with a buckle, and a ribbon marker hanging out.
    p.span((0.04, -h / 2 - 0.006, -0.0045), (0.065, h / 2 + 0.006, 0.0), "Wood dark")
    p.span((0.04, -h / 2 - 0.006, -0.0045), (0.065, -h / 2 - 0.001, t + 0.0045), "Wood dark")
    p.span((0.04, h / 2 + 0.001, -0.0045), (0.065, h / 2 + 0.006, t + 0.0045), "Wood dark")
    p.span((0.04, -h / 2 - 0.006, t), (0.065, h / 2 + 0.006, t + 0.0045), "Wood dark")
    p.torus(0.016, 0.0022, (0.0525, -0.06, t + 0.0067), "Brass", segments=14, sides=5)
    p.span((0.051, -0.076, t + 0.0045), (0.054, -0.044, t + 0.0095), "Brass")
    p.box((0.007, 0.06, 0.0012), (-0.05, -h / 2 - 0.036, 0.0006), "Ribbon red", rot=(0, 0, 6))
    # The label plate on the front cover, and the ledger's number.
    p.paint_up((-0.06, 0.04), (0.02, 0.09), t, "Brass")
    p.paint_up((-0.055, 0.045), (0.015, 0.085), t, "Leather", layer=2)
    p.lettering("88", (-0.02, 0.065, t), 0.024, "Brass", layer=3)
    return p


def land_claim(style: Style) -> Item:
    """A land claim: a deed rolled and tied with ribbon, its seal hanging
    off it. Cheap now, a fortune if the charter settles the right way."""
    p = Item("land_claim", "32 cm roll, 5.5 cm across")
    r, ln = 0.027, 0.32
    # The roll, a hollow core, and the deed's loose end lying out flat.
    p.lathe([(0.0, -ln / 2), (r - 0.002, -ln / 2), (r, -ln / 2 + 0.002), (r, ln / 2 - 0.002), (r - 0.002, ln / 2), (0.0, ln / 2)],
            (0, 0, r), "Paper aged", rot=(0, 90, 0), segments=20)
    for sx in (-1, 1):
        with p.painted():
            p.cyl(0.008, THICK, (sx * (ln / 2 + lift(1)), 0, r), "Wood dark", rot=(0, 90, 0), segments=16)
            for rr in (0.013, 0.019, 0.024):
                p.torus(rr, 0.0006, (sx * (ln / 2 + lift(1)), 0, r), "Paper", rot=(0, 90, 0), segments=20, sides=ROUND)
    p.cloth([(-ln / 2, -r * 0.9, 0.0012), (ln / 2, -r * 0.9, 0.0012), (ln / 2, -r - 0.06, 0.002), (-ln / 2, -r - 0.06, 0.002)],
            "Paper aged", sag=0.0, ripple=0.0008, thickness=0.0008, cell=0.03)
    # The ribbon, a bow and two tails, and the seal on its cord.
    p.torus(r + 0.003, 0.0035, (0.03, 0, r), "Ribbon red", rot=(0, 90, 0), segments=20, sides=6)
    for side in (-1, 1):
        p.torus(0.014, 0.0035, (0.03, side * 0.012, 2 * r + 0.006), "Ribbon red", rot=(0, 90 - side * 20, 90), segments=14, sides=5)
    with p.painted():
        p.box((0.008, 0.07, THICK), (0.04, -r - 0.03, 0.002 + lift(1)), "Ribbon red", rot=(0, 0, 18))
        p.box((0.008, 0.065, THICK), (0.018, -r - 0.028, 0.002 + lift(1)), "Ribbon red", rot=(0, 0, -12))
    p.lathe([(0.0, 0.0), (0.02, 0.0), (0.021, 0.002), (0.018, 0.005), (0.0, 0.0055)], (0.06, -r - 0.1, 0.0), "Wax red", segments=18)
    p.torus(0.011, 0.0016, (0.06, -r - 0.1, 0.0055), "Wax red", segments=16, sides=5)
    p.lettering("12", (0.06, -r - 0.1, 0.0055), 0.008, "Ink")
    return p


def registrar_stamp(style: Style) -> Item:
    """A Registrar's stamp: what makes a contract filed. A turned wooden
    handle on a brass body, the Exchange's seal cut in its rubber face."""
    p = Item("registrar_stamp", "7.5 cm across, 12.5 cm high")
    # Rubber die, its mount, the knurled brass body.
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.036, 0.002), (0.036, 0.009), (0.0, 0.009)], (0, 0, 0), "Rubber", segments=14)
    # (Its knurled rings are ridges in its profile.)
    p.lathe([(0.0, 0.009), (0.038, 0.009), (0.039, 0.012), (0.0395, 0.014), (0.039, 0.016), (0.0395, 0.018), (0.039, 0.02),
             (0.0395, 0.022), (0.039, 0.024), (0.039, 0.026), (0.036, 0.029), (0.024, 0.034),
             (0.016, 0.042), (0.013, 0.06), (0.016, 0.066), (0.0, 0.068)], (0, 0, 0), "Brass", segments=14)
    # The handle: a turned knob with a ferrule.
    p.lathe([(0.0, 0.066), (0.015, 0.066), (0.017, 0.072), (0.012, 0.08), (0.016, 0.092), (0.024, 0.104),
             (0.026, 0.113), (0.021, 0.122), (0.01, 0.126), (0.0, 0.127)], (0, 0, 0), "Wood", segments=12)
    p.torus(0.0155, 0.0022, (0, 0, 0.07), "Brass", segments=10, sides=ROUND)
    # A notch so it goes the right way up, and the Exchange's mark on its side.
    p.box((0.008, 0.008, 0.012), (0, -0.039, 0.021), "Charter navy")
    with p.painted():
        p.box((0.022, THICK, 0.012), (0, 0.039 + lift(1), 0.019), "Charter navy")
        p.box((0.014, THICK, 0.006), (0, 0.039 + lift(2), 0.019), "Brass")
    return p


def forged_seal(style: Style) -> Item:
    """A forged Registrar seal: contraband, a crude copy carved from an
    offcut, its handle a bit of pipe wrapped in tape."""
    p = Item("forged_seal", "6.5 cm across, 13 cm high; contraband")
    p.box((0.06, 0.056, 0.012), (0.002, 0, 0.006), "Rubber", rot=(0, 0, 4))
    p.box((0.068, 0.064, 0.024), (0, 0, 0.024), "Wood", rot=(0, 0, 4))
    # Saw marks and a gouge on the block: paint.
    with p.painted():
        for i in range(4):
            p.box((0.06, THICK, 0.0012), (0.002, -0.034 - LIFT, 0.016 + i * 0.004), "Wood dark", rot=(0, 0, 4))
    # The handle leans, as nobody measured it; the tape follows it.
    lean = (5, -3, 0)
    axis = _euler(lean).to_3x3() @ Vector((0, 0, 1))
    foot = Vector((0.003, 0, 0.036))
    p.cyl(0.012, 0.09, foot + axis * 0.045, "Hull alloy", rot=lean, segments=8)
    p.cyl(0.007, 0.102, foot + axis * 0.045, "Hull dark", rot=lean, segments=6)
    with p.painted():
        for k, ln in ((0.02, 0.012), (0.036, 0.014), (0.06, 0.012)):
            p.cyl(0.012 + lift(1) + THICK, ln, foot + axis * k, "Tape", rot=lean, segments=12)
    # Nails holding it together, and a bodge of copper wire round its neck.
    for x, y in ((-0.024, -0.022), (0.026, 0.02), (-0.021, 0.025)):
        p.cyl(0.0045, 0.005, (x, y, 0.0385), "Steel", segments=5)
    p.torus(0.016, 0.0014, (0.003, 0, 0.04), "Copper", segments=8, sides=ROUND)
    p.torus(0.0142, 0.0014, foot + axis * 0.012, "Copper", rot=lean, segments=8, sides=ROUND)
    return p


# Air and water -------------------------------------------------------------

def rebreather(style: Style) -> Item:
    """A rebreather: a rubber half mask with a filter can on each cheek and a
    harness behind. Outside the domes nobody goes without one. It sits on its
    chin, its straps fallen behind it."""
    p = Item("rebreather", "a half mask, 0.26 m across the cans")
    cz = 0.07
    # The face cup, its seal, and the nose bridge.
    p.lathe([(0.058, 0.0), (0.066, 0.002), (0.067, 0.015), (0.06, 0.035), (0.046, 0.052), (0.026, 0.063), (0.0, 0.067)],
            (0, 0.018, cz), "Rubber", rot=(90, 0, 0), segments=12)
    p.torus(0.062, 0.006, (0, 0.016, cz), "Hull dark", rot=(90, 0, 0), segments=12, sides=ROUND)
    p.sphere(0.024, (0, -0.0, cz + 0.05), "Rubber", scale=(1, 0.8, 0.6), segments=8, rings=4)
    # The exhale valve: a domed cover with slots.
    p.lathe([(0.0, 0.0), (0.024, 0.0), (0.026, 0.006), (0.022, 0.014), (0.012, 0.019), (0.0, 0.02)],
            (0, -0.042, cz - 0.012), "Hull dark", rot=(90, 0, 0), segments=10)
    with p.painted():
        for i in range(4):
            p.box((0.026, THICK, 0.0025), (0, -0.064 - LIFT, cz - 0.02 + i * 0.0055), "Ink")
    # The cans, threaded into ports on each cheek, angled out and forward.
    for side in (-1, 1):
        rot = (90, 0, side * 38)
        d = _euler(rot).to_3x3() @ Vector((0, 0, 1))
        port = Vector((side * 0.05, -0.02, cz - 0.008))
        p.cyl(0.02, 0.022, port + d * 0.011, "Rubber", rot=rot, segments=8)
        can = port + d * 0.022
        # (Its grooves are in its profile; the dark of them is paint.)
        p.lathe([(0.0, 0.0), (0.036, 0.0), (0.04, 0.004), (0.04, 0.046), (0.036, 0.05), (0.0, 0.05)],
                can, "Crew orange", rot=rot, segments=12)
        with p.painted():
            for k in (0.012, 0.024, 0.036):
                p.cyl(0.04 + lift(1) + THICK, 0.0025, can + d * k, "Hull dark", rot=rot, segments=12)
        face = can + d * (0.05 + LIFT)
        with p.painted():
            p.cyl(0.03, THICK, face, "Filter media", rot=rot, segments=20)
            for i in range(-2, 3):
                for j in range(-2, 3):
                    if i * i + j * j <= 5:
                        off = _euler(rot).to_3x3() @ Vector((i * 0.01, j * 0.01, THICK))
                        p.cyl(0.0022, THICK, face + off, "Ink", rot=rot, segments=6)
    # The harness: a crown strap and a neck strap, buckled, fallen behind.
    for side in (-1, 1):
        for zz in (cz + 0.027, cz - 0.033):
            p.torus(0.009, 0.0022, (side * 0.064, 0.016, zz), "Steel", rot=(0, 90, 0), segments=ROUND, sides=ROUND)
    p.tube([(-0.064, 0.02, cz + 0.027), (-0.075, 0.09, 0.02), (0.0, 0.15, 0.006),
            (0.075, 0.09, 0.02), (0.064, 0.02, cz + 0.027)], 0.006, "Hull dark", segments=ROUND)
    p.tube([(-0.064, 0.02, cz - 0.033), (-0.06, 0.06, 0.008), (0.06, 0.06, 0.008),
            (0.064, 0.02, cz - 0.033)], 0.005, "Hull dark", segments=ROUND)
    return p


def water_canister(style: Style) -> Item:
    """A water canister: twenty litres in a pressed-steel jerrycan, painted
    the Crew's water blue, a brass tag on its handle. Water rights are filed;
    the water itself is carried."""
    p = Item("water_canister", "20 litre jerrycan, 0.5 m high")
    p.source("metal_jerrycan", height=0.5, recolour="Water blue")
    # The tag on its wire, off the front handle.
    p.cable((0.02, -0.03, 0.505), (0.03, -0.075, 0.45), 0.0012, "Steel", sag=0.005, segments=6, steps=6)
    p.box((0.03, 0.002, 0.04), (0.03, -0.077, 0.43), "Brass", rot=(0, 0, 8))
    return p


# Food ----------------------------------------------------------------------

def grain_sack(style: Style) -> Item:
    """A sack of Fringe grain: cheap outside the domes, dear on Charter Row.
    Slumped on its bottom, its neck gathered and tied with twine."""
    p = Item("grain_sack", "about 25 kg, 0.46 × 0.34 × 0.62 m")
    bm = p.bm
    r = bmesh.ops.create_uvsphere(bm, u_segments=28, v_segments=14, radius=1.0)
    verts = r["verts"]
    for v in verts:
        x, y, z = v.co
        # A filled sack, settled: a rounded foot spread a little where it
        # sits, straight sides, shoulders drawn in to the gathered neck.
        zz = (z + 1) / 2
        if zz < 0.15:
            k = (zz / 0.15) ** 0.45
        elif zz < 0.62:
            k = 1.0
        else:
            k = 1 - 0.78 * ((zz - 0.62) / 0.38) ** 1.25
        k *= 1 + 0.1 * (1 - zz) ** 3
        rr = math.hypot(x, y) or 1.0
        v.co = Vector((x / rr * 0.21 * k if rr > 1e-6 else 0.0, y / rr * 0.145 * k if rr > 1e-6 else 0.0, zz * 0.53))
    p._take(verts, "Canvas")
    for f in {f for v in verts for f in v.link_faces}:
        f.smooth = True
    # The gathered neck, its folds, the twine and the tuft above.
    n = 10
    for k in range(n):
        a = math.tau * k / n
        p.cyl(0.013, 0.08, (math.cos(a) * 0.03, math.sin(a) * 0.026, 0.55), "Canvas", rot=(math.sin(a) * 10, -math.cos(a) * 10, 0), segments=ROUND, radius2=0.02)
    p.torus(0.036, 0.005, (0, 0, 0.56), "Twine", segments=10, sides=ROUND)
    p.torus(0.034, 0.004, (0, 0, 0.57), "Twine", rot=(0, 8, 0), segments=10, sides=ROUND)
    p.cable((0.035, -0.01, 0.565), (0.07, -0.07, 0.49), 0.003, "Twine", sag=-0.01, segments=ROUND, steps=4)
    # The ears of the gathered top, splayed.
    for a in (0, 120, 240):
        rad = math.radians(a)
        p.sphere(0.03, (math.cos(rad) * 0.03, math.sin(rad) * 0.03, 0.6), "Canvas", scale=(1.3, 0.5, 0.8), rot=(0, 0, a), segments=6, rings=4)
    # The grower's stamp: a wheat sheaf mark and a lot number.
    front = -0.15
    with p.painted():
        for (cx, cz, sw, sh) in ((0, 0.285, 0.11, 0.006), (0, 0.195, 0.11, 0.006), (-0.052, 0.24, 0.006, 0.096), (0.052, 0.24, 0.006, 0.096)):
            p.box((sw, THICK, sh), (cx, front - lift(1), cz), "Ribbon red")
    p.stencil_on("07", 0, front, 0.24, 0.05, "Ribbon red")
    return p


def produce_crate(style: Style) -> Item:
    """A crate of dome produce, heaped: apples, lemons, onions and sweet
    potatoes grown under glass, sold by the kilo."""
    p = Item("produce_crate", "0.5 × 0.4 × 0.3 m crate, heaped")
    p.source("plastic_crate_02", length=0.5)
    r = rng(style, p.name)
    heap = [("food_apple_01", 0.075), ("lemon", 0.07), ("yellow_onion", 0.08), ("sweet_potato", 0.075)]
    # A layer filling the crate to its rim, and a few on top of it.
    spots = [(-0.17 + i * 0.113, -0.12 + j * 0.12, 0.185) for i in range(4) for j in range(3)]
    spots += [(-0.11, -0.04, 0.245), (0.0, 0.05, 0.25), (0.11, -0.03, 0.245), (0.04, -0.09, 0.24)]
    for k, (x, y, z) in enumerate(spots):
        asset, size = heap[(k + r.randrange(2)) % len(heap)]
        p.source(asset, at=(x + r.uniform(-0.012, 0.012), y + r.uniform(-0.01, 0.01), z),
                 rot=(r.uniform(-25, 25), r.uniform(-25, 25), r.uniform(0, 360)), height=size)
    return p


def protein_pack(style: Style) -> Item:
    """A pack of protein ration bricks from the Hull's vats: six in a foil
    tray, banded with the Crew's label. Nobody likes them; everybody eats
    them."""
    p = Item("protein_pack", "0.25 × 0.17 × 0.07 m")
    w, d = 0.25, 0.17
    # The tray: a pressed foil dish with sloped walls and a rolled rim.
    flat(p, rounded(w - 0.02, d - 0.02, 0.015), 0.004, (0, 0, 0), "Foil")
    for sx in (-1, 1):
        p.box((0.004, d - 0.01, 0.03), (sx * (w / 2 - 0.007), 0, 0.017), "Foil", rot=(0, sx * 10, 0))
    for sy in (-1, 1):
        p.box((w - 0.01, 0.004, 0.03), (0, sy * (d / 2 - 0.007), 0.017), "Foil", rot=(sy * -10, 0, 0))
    p.tube([(-w / 2, -d / 2, 0.032), (w / 2, -d / 2, 0.032), (w / 2, d / 2, 0.032), (-w / 2, d / 2, 0.032), (-w / 2, -d / 2, 0.032)],
           0.0025, "Foil", segments=6)
    # Six bricks, rounded, scored across their tops.
    for i in range(3):
        for j in range(2):
            x, y = (i - 1) * 0.077, (j - 0.5) * 0.072
            p.span((x - 0.035, y - 0.033, 0.004), (x + 0.035, y + 0.033, 0.058), "Protein")
            for k in (-1, 1):
                p.paint_up((x + k * 0.012 - 0.0008, y - 0.03), (x + k * 0.012 + 0.0008, y + 0.03), 0.058, "Paper aged")
    # The Crew's band across, its count.
    with p.painted():
        p.span((-0.03, -d / 2 - lift(2) - THICK, -lift(2) - THICK), (0.03, d / 2 + lift(2) + THICK, 0.058 + lift(2) + THICK), "Crew orange")
    p.lettering("6", (0, 0, 0.058), 0.03, "Stencil white", layer=3)
    return p


def coffee_tin(style: Style) -> Item:
    """A tin of real Earth coffee, off a drifter: a luxury, and a bribe that
    always works on Charter Row. Rolled rims, a key-opened lid, its label
    worn at the edges."""
    p = Item("coffee_tin", "12 cm across, 17 cm high")
    r, h = 0.06, 0.155
    p.lathe([(0.0, 0.0), (r - 0.003, 0.0), (r, 0.004), (r, h - 0.004), (r - 0.003, h), (0.0, h)], (0, 0, 0), "Steel", segments=14)
    for z in (0.004, h - 0.004):
        p.torus(r + 0.001, 0.003, (0, 0, z), "Steel", segments=14, sides=ROUND)
    # The lid, a little proud, and its winding key.
    p.lathe([(0.0, h), (r + 0.003, h), (r + 0.004, h + 0.004), (r + 0.003, h + 0.01), (0.0, h + 0.01)], (0, 0, 0), "Brass", segments=14)
    p.tube([(-0.05, 0, h + 0.0135), (0.04, 0, h + 0.0135)], 0.0035, "Steel", segments=ROUND)
    p.torus(0.01, 0.0025, (0.052, 0, h + 0.0135), "Steel", segments=ROUND, sides=ROUND)
    # The label: dark roast brown, a cream band with the bean, gold lines.
    for z0, z1, mat in ((0.028, 0.071, "Wood dark"), (0.071, 0.074, "Brass"), (0.074, 0.106, "Paper"),
                        (0.106, 0.109, "Brass"), (0.109, 0.127, "Wood dark")):
        p.sleeve(r, z0, z1, mat)
    with p.painted():
        p.sphere(0.012, (0, -r - lift(2) - 0.002, 0.09), "Wood", scale=(0.75, 0.15, 1), segments=12, rings=6)
        p.box((0.0015, THICK, 0.018), (0, -r - lift(3) - 0.002, 0.09), "Wood dark")
    return p


# Parts ---------------------------------------------------------------------

def power_cell(style: Style) -> Item:
    """A power cell: the Hull's batteries; nobody can make new ones, and a
    laser burns through them. Hexagonal, rubber-bumpered, its charge lit
    down its face."""
    p = Item("power_cell", "14 cm across the flats, 25 cm high")
    R = 0.068
    flat_r = R * math.cos(math.radians(30))
    ngon(p, 6, R, 0.17, (0, 0, 0.115), "Crew grey", rot=(0, 0, HEX))
    for z in (0.02, 0.21):
        ngon(p, 6, 0.076, 0.04, (0, 0, z), "Rubber", rot=(0, 0, HEX))
    with p.painted():
        for z in (0.08, 0.15):
            ngon(p, 6, (flat_r + lift(1) + THICK) / math.cos(math.radians(30)), 0.012, (0, 0, z), "Hull dark", rot=(0, 0, HEX))
    # Terminals with hex nuts, and the carrying bail.
    for x in (-0.028, 0.028):
        p.cyl(0.013, 0.005, (x, 0, 0.2325), "Wax red" if x > 0 else "Hull dark", segments=12)
        ngon(p, 6, 0.012, 0.006, (x, 0, 0.238), "Steel")
        p.cyl(0.0085, 0.022, (x, 0, 0.2515), "Copper", segments=12)
    p.tube([(-0.05, 0.025, 0.226), (-0.05, 0.025, 0.262), (0.05, 0.025, 0.262), (0.05, 0.025, 0.226)], 0.006, "Hull dark", segments=6)
    # The charge window: four bars, three lit; a warning plate; its rating.
    face = -flat_r
    p.paint_front((-0.024, 0.06), (0.024, 0.14), face, "Screen", layer=2)
    for i in range(4):
        p.paint_front((-0.017, 0.064 + i * 0.019), (0.017, 0.076 + i * 0.019), face, "Laser cyan" if i < 3 else "Hull alloy", layer=3)
    p.paint_front((-0.026, 0.16), (0.026, 0.18), face, "Hazard yellow")
    p.stencil_on("40", 0, face, 0.17, 0.014, "Ink", layer=2)
    return p


def terraformer_part(style: Style) -> Item:
    """A terraformer component: a coil stack and its rotor, stripped from one
    of the wrecks on the horizon. Precision work nobody can make any more."""
    p = Item("terraformer_part", "0.34 m across, 0.42 m high")
    # The base flange, bolted, and the coil's mount.
    p.lathe([(0.0, 0.0), (0.165, 0.0), (0.168, 0.004), (0.168, 0.02), (0.16, 0.024), (0.09, 0.026), (0.088, 0.05), (0.0, 0.05)],
            (0, 0, 0), "Hull dark", segments=12)
    for i in range(4):
        a = math.tau * i / 4 + math.pi / 4
        ngon(p, 6, 0.011, 0.008, (math.cos(a) * 0.14, math.sin(a) * 0.14, 0.028), "Steel", rot=(0, 0, 15))
    # The windings: a ridged lathe of copper on a dark core.
    # (Gentle ridges: any sharper and the finish bevels every one.)
    prof = [(0.0, 0.05), (0.058, 0.05)]
    for k in range(25):
        z = 0.055 + k * 0.0075
        prof += [(0.0695 if k % 2 == 0 else 0.0675, z)]
    prof += [(0.058, 0.245), (0.0, 0.245)]
    p.lathe(prof, (0, 0, 0), "Copper", segments=12)
    p.lathe([(0.0, 0.245), (0.088, 0.245), (0.09, 0.25), (0.088, 0.262), (0.0, 0.262)], (0, 0, 0), "Hull alloy", segments=14)
    # Leads from the coil down to the terminal block.
    p.span((0.09, -0.03, 0.026), (0.125, 0.03, 0.06), "Hull dark")
    for k, y in enumerate((-0.015, 0.015)):
        p.cable((0.068, y, 0.2 - k * 0.04), (0.12, y, 0.062), 0.004, "Wax red" if k else "Rubber", sag=-0.01, segments=ROUND, steps=4)
    # Shaft, hub, and six pitched blades.
    p.cyl(0.014, 0.09, (0, 0, 0.3), "Steel", segments=ROUND)
    p.lathe([(0.0, 0.33), (0.034, 0.33), (0.036, 0.345), (0.03, 0.37), (0.012, 0.385), (0.0, 0.388)], (0, 0, 0), "Corvane white", segments=10)
    for i in range(6):
        a = 360 * i / 6
        p.prism([(0.03, -0.014), (0.11, -0.016), (0.162, -0.006), (0.165, 0.01), (0.11, 0.018), (0.03, 0.016)], 0.005,
                (0, 0, 0.35), "Corvane white", rot=(28, 0, a))
    p.sleeve(0.0365, 0.336, 0.342, "Corvane blue")
    p.paint_front((0.09, 0.033), (0.125, 0.052), -0.03, "Hazard yellow")
    return p


def salvage_scrap(style: Style) -> Item:
    """A bundle of salvage: bent hull plate, pipe and a rusted wheel rim,
    roped together to carry in from the salvage fields and sell by weight."""
    p = Item("salvage_scrap", "about 0.65 × 0.35 × 0.3 m")
    r = rng(style, p.name)
    mats = ["Hull alloy", "Rust", "Repaint teal", "Repaint oxide", "Hull dark"]
    # Bent plates, each a sheet bowed out of true.
    z = 0.01
    for i in range(4):
        ln, wd = r.uniform(0.45, 0.6), r.uniform(0.2, 0.28)
        x0, y0 = r.uniform(-0.03, 0.03), r.uniform(-0.02, 0.02)
        dz = r.uniform(-0.02, 0.02)
        p.cloth([(x0 - ln / 2, y0 - wd / 2, z + dz), (x0 + ln / 2, y0 - wd / 2, z - dz), (x0 + ln / 2, y0 + wd / 2, z + 0.02),
                 (x0 - ln / 2, y0 + wd / 2, z)], mats[i], sag=-0.03, ripple=0.004, thickness=0.006, cell=0.05, seed=i)
        z += 0.04
    # Pipe lengths along it, one with a flange.
    p.tube([(-0.32, -0.09, z + 0.01), (0.15, -0.1, z + 0.02), (0.31, -0.05, z + 0.08)], 0.022, "Hull dark", segments=10)
    p.cyl(0.034, 0.012, (-0.31, -0.09, z + 0.01), "Rust", rot=(0, 90, 0), segments=12)
    p.tube([(-0.28, 0.09, z), (0.31, 0.11, z - 0.01)], 0.014, "Copper", segments=8)
    # A wheel rim thrown on top.
    p.source("rusted_wheel_rim_02", at=(0.06, 0.02, z + 0.03), rot=(90, 0, 20), height=0.1)
    # Rope round it all, twice.
    for x in (-0.17, 0.17):
        p.torus(0.17, 0.007, (x, 0, 0.14), "Twine", rot=(0, 90, 0), segments=18, sides=6)
    return p


def valve(style: Style) -> Item:
    """A replacement gate valve: what a repair run carries down to the lower
    decks. Flanged at both ends, bolted bonnet, its handwheel on a yoke."""
    p = Item("valve", "0.32 m long, 0.36 m high")
    rf = 0.075
    # The body, cast and flanged.
    p.lathe([(0.0, -0.11), (0.045, -0.11), (0.05, -0.08), (0.066, -0.04), (0.07, 0.0), (0.066, 0.04), (0.05, 0.08), (0.045, 0.11), (0.0, 0.11)],
            (0, 0, rf), "Crew orange", rot=(0, 90, 0), segments=12)
    for sx in (-1, 1):
        p.lathe([(0.03, -0.012), (rf, -0.012), (rf, 0.012), (0.03, 0.012)], (sx * 0.135, 0, rf), "Hull dark", rot=(0, 90, 0), segments=10)
        for i in range(4):
            a = math.tau * i / 4 + math.pi / 4
            ngon(p, 6, 0.008, 0.034, (sx * 0.135, math.cos(a) * 0.055, rf + math.sin(a) * 0.055), "Steel", rot=(0, 90, 0))
    # The bonnet, bolted to the body, and the yoke over it.
    p.lathe([(0.0, 0.0), (0.05, 0.0), (0.05, 0.012), (0.036, 0.016), (0.03, 0.06), (0.0, 0.06)], (0, 0, rf + 0.05), "Crew orange", segments=12)
    with p.painted():
        for i in range(6):
            a = math.tau * i / 6
            p.cyl(0.0055, THICK, (math.cos(a) * 0.043, math.sin(a) * 0.043, rf + 0.064 + lift(1)), "Steel", segments=6)
    for sx in (-1, 1):
        p.box((0.012, 0.022, 0.07), (sx * 0.022, 0, rf + 0.14), "Crew orange", rot=(0, sx * -8, 0))
    p.span((-0.03, -0.012, rf + 0.172), (0.03, 0.012, rf + 0.186), "Crew orange")
    # The threaded stem, the handwheel and its spokes.
    p.cyl(0.008, 0.128, (0, 0, rf + 0.13), "Steel", segments=ROUND)
    for k in range(4):
        p.sleeve(0.008, rf + 0.145 + k * 0.006, rf + 0.147 + k * 0.006, "Hull dark", segments=12)
    p.torus(0.075, 0.009, (0, 0, rf + 0.205), "Valve red", segments=14, sides=ROUND)
    for i in range(3):
        p.box((0.15, 0.01, 0.008), (0, 0, rf + 0.205), "Valve red", rot=(0, 0, 60 * i))
    p.cyl(0.016, 0.022, (0, 0, rf + 0.205), "Hull dark", segments=8)
    # Its tag, hung off the yoke.
    p.cable((0.03, 0.0, rf + 0.18), (0.055, -0.02, rf + 0.12), 0.0012, "Steel", sag=0.004, segments=ROUND, steps=3)
    p.box((0.025, 0.002, 0.035), (0.058, -0.022, rf + 0.1), "Brass", rot=(0, 0, 10))
    p.stencil_on("14", 0.058, -0.0235, rf + 0.1, 0.012, "Ink")
    p.paint_front((-0.035, rf - 0.02), (0.035, rf + 0.02), -0.07, "Stencil white")
    p.stencil_on("65", 0, -0.07, rf, 0.022, "Ink", layer=2)
    return p


def data_core(style: Style) -> Item:
    """A Corvane data core: clean, new and pressed, and contraband. Company
    records nobody here was meant to have; the Receivership will want them."""
    p = Item("data_core", "7.5 cm across, 19 cm high; contraband")
    p.lathe([(0.0, 0.0), (0.032, 0.0), (0.037, 0.005), (0.037, 0.07), (0.035, 0.074), (0.035, 0.112), (0.037, 0.116),
             (0.037, 0.155), (0.033, 0.168), (0.02, 0.182), (0.0, 0.186)], (0, 0, 0), "Corvane white", segments=14)
    # The blue band at its waist, its contact rings, and the company's mark.
    p.sleeve(0.035, 0.074, 0.112, "Corvane blue")
    for z in (0.008, 0.016, 0.024):
        p.sleeve(0.037, z - 0.0015, z + 0.0015, "Brass")
    # The lattice window, lit.
    p.paint_front((-0.013, 0.118), (0.013, 0.152), -0.037, "Screen")
    for i in range(3):
        p.paint_front((-0.008, 0.122 + i * 0.01), (0.008, 0.127 + i * 0.01), -0.037, "Laser cyan", layer=2)
    p.paint_up((-0.011, -0.011), (0.011, 0.011), 0.186, "Corvane blue")
    p.lettering("90", (0, 0, 0.186), 0.008, "Corvane white", layer=2)
    return p


# Off-world goods -----------------------------------------------------------

def medkit(style: Style) -> Item:
    """A medkit off a drifter: Earth medicine in a white first-aid tin, worth
    more than its weight in marks anywhere but the Pads on the day it lands."""
    p = Item("medkit", "0.34 × 0.23 m tin")
    p.source("medical_box", length=0.34, recolour="Medical white")
    return p


def seed_case(style: Style) -> Item:
    """A sealed case of seed vials from Earth: next year's crops for whoever
    can afford them. Corvane white, its lid open on its hinge, the vials in
    their foam."""
    p = Item("seed_case", "0.34 × 0.24 m case, lid open")
    w, d, h = 0.17, 0.115, 0.075
    wall = 0.01
    p.span((-w, -d, 0.006), (w, d, 0.02), "Corvane white")
    for sx in (-1, 1):
        p.span((sx * w - (wall if sx > 0 else 0), -d, 0.006), (sx * w + (wall if sx < 0 else 0), d, h), "Corvane white")
    for sy in (-1, 1):
        p.span((-w, sy * d - (wall if sy > 0 else 0), 0.006), (w, sy * d + (wall if sy < 0 else 0), h), "Corvane white")
    p.span((-w - 0.005, -d - 0.005, h - 0.012), (w + 0.005, d + 0.005, h + 0.005), "Corvane blue")
    for sx in (-1, 1):
        for sy in (-1, 1):
            p.span((sx * (w - 0.025) - 0.012, sy * (d - 0.02) - 0.01, 0.0), (sx * (w - 0.025) + 0.012, sy * (d - 0.02) + 0.01, 0.006), "Rubber")
    p.span((-w + wall, -d + wall, 0.02), (w - wall, d - wall, h - 0.015), "Hull dark")
    # Twelve vials: seeds in glass, the caps the colours of what's in them.
    caps = ["Crop green", "Wheat", "Medical red", "Container green", "Crew orange", "Fabric olive"]
    r = rng(style, p.name)
    for i in range(6):
        for j in range(2):
            x, y = -0.125 + i * 0.05, (j - 0.5) * 0.09
            # Seed showing through the glass, and the cap.
            p.cyl(0.0135, 0.072, (x, y, h + 0.001), ["Seed", "Wheat", "Wood"][(i + j + r.randrange(2)) % 3], segments=ROUND)
            p.cyl(0.0155, 0.016, (x, y, h + 0.045), caps[(i + 2 * j) % len(caps)], segments=ROUND)
    # The lid, hinged at the back and stood open; latches and a handle.
    p.box((2 * w, 0.028, 2 * d), (0, d + 0.017, h + d - 0.01), "Corvane white", rot=(-8, 0, 0))
    for x in (-0.12, 0.12):
        p.cyl(0.007, 0.04, (x, d + 0.004, h), "Steel", rot=(0, 90, 0), segments=5)
    for x in (-0.1, 0.1):
        p.span((x - 0.014, -d - 0.011, h - 0.035), (x + 0.014, -d, h - 0.008), "Steel")
    p.tube([(-0.05, -d - 0.016, 0.035), (0.05, -d - 0.016, 0.035)], 0.007, "Hull dark", segments=ROUND)
    for x in (-0.05, 0.05):
        p.span((x - 0.006, -d - 0.02, 0.028), (x + 0.006, -d, 0.042), "Hull dark")
    p.paint_front((-0.1, 0.012), (0.1, 0.022), -d, "Corvane blue")
    p.lid_paint((0, d + 0.017, h + d - 0.01), -8, (2 * w, 0.028, 2 * d), [(0, 0.03, 0.12, 0.06)], "Corvane blue")
    return p


def electronics_box(style: Style) -> Item:
    """A box of off-world electronics in the drifter's taped carton, a board
    lying on top and a coil of cable: whoever has them can fix what's broken."""
    p = Item("electronics_box", "0.4 × 0.5 × 0.3 m carton")
    p.source("cardboard_box_01", length=0.4)
    p.source("circuit_board", at=(0.0, 0.02, 0.305), rot=(0, 0, 14), length=0.24)
    p.torus(0.045, 0.007, (0.12, -0.15, 0.31), "Copper", segments=16, sides=6)
    p.torus(0.035, 0.006, (0.12, -0.15, 0.322), "Rubber", segments=16, sides=6)
    p.stencil_on("01", 0.0, -0.25, 0.15, 0.07, "Medical red")
    return p


def wine_bottle(style: Style) -> Item:
    """A bottle of Earth wine: a luxury off a drifter, opened only on Charter
    Row, or kept unopened and sold on."""
    p = Item("wine_bottle", "7.5 cm across, 30 cm high")
    # The second of Poly Haven's four: the red, its label and capsule.
    p.crop("wine_bottles_01", ((0.17, 0.25), (-0.05, 0.05)), height=0.3, glass="Wine dark")
    return p


# Tools ---------------------------------------------------------------------

def wrench(style: Style) -> Item:
    """A combination spanner: every Crew hand carries one."""
    p = Item("wrench", "0.3 m long, lying flat")
    p.source("combination_wrench", rot=(0, 0, 90), length=0.3)
    return p


def toolbox(style: Style) -> Item:
    """A Crew toolbox: steel, painted the Crew's orange over whatever it was,
    the owner's number stencilled on the front, the lid open."""
    p = Item("toolbox", "0.46 m long, lid open")
    p.source("metal_toolbox", length=0.46, recolour="Crew orange")
    p.stencil_on("117", -0.12, -0.143, 0.1, 0.045, "Stencil white")
    return p


def crowbar(style: Style) -> Item:
    """A crowbar, painted Crew orange: for crates, hatches and salvage, and
    for persuasion."""
    p = Item("crowbar", "0.62 m long, lying flat")
    p.source("crowbar_01", rot=(0, 90, 0), length=0.62, recolour="Crew orange")
    return p


def flashlight(style: Style) -> Item:
    """A plastic torch: the lower decks are dark where the lights have
    failed."""
    p = Item("flashlight", "0.24 m long, lying on its side")
    p.source("small_plastic_torch", rot=(0, 0, 90), length=0.24)
    return p


def radio(style: Style) -> Item:
    """A handheld radio: how contractors talk across Landfall, and how the
    Quiet Book listens. Rubber-armoured, its aerial bent from use."""
    p = Item("radio", "6.5 × 4 × 17 cm, plus aerial")
    w, d, h = 0.033, 0.02, 0.15
    p.span((-w, -d, 0.006), (w, d, h), "Gunmetal")
    # Rubber armour at the foot and round the shoulders.
    p.span((-w - 0.005, -d - 0.005, 0.0), (w + 0.005, d + 0.005, 0.03), "Rubber")
    p.span((-w - 0.005, -d - 0.005, h - 0.02), (w + 0.005, d + 0.005, h + 0.006), "Rubber")
    for sx in (-1, 1):
        x0, x1 = sorted((sx * w - sx * 0.004, sx * w + sx * 0.005))
        p.span((x0, -d - 0.005, 0.03), (x1, d + 0.005, h - 0.02), "Rubber")
    # The keypad: keys in the body's own colour, their faces painted.
    for i in range(3):
        for j in range(3):
            x, z = -0.017 + i * 0.017, 0.038 + j * 0.011
            p.span((x - 0.006, -d - 0.002, z - 0.004), (x + 0.006, -d, z + 0.004), "Gunmetal")
            p.paint_front((x - 0.004, z - 0.002), (x + 0.004, z + 0.002), -d - 0.002, "Hull alloy" if (i, j) != (1, 0) else "Crew orange")
    # The speaker grille and the screen with its channel.
    for k in range(6):
        p.paint_front((-0.021, 0.0775 + k * 0.004), (0.021, 0.0795 + k * 0.004), -d, "Ink")
    p.paint_front((-0.024, 0.105), (0.024, 0.128), -d, "Status blue")
    p.stencil_on("17", 0, -d, 0.1165, 0.014, "Ink", layer=2)
    # Push-to-talk on its side, a belt clip behind.
    p.span((-w - 0.011, -0.009, 0.075), (-w - 0.004, 0.009, 0.11), "Crew orange")
    p.span((-0.012, d - 0.002, 0.05), (0.012, d + 0.006, h - 0.025), "Hull dark")
    # Knobs on top, and the aerial, a little bent.
    p.cyl(0.0085, 0.016, (0.017, 0, h + 0.012), "Hull dark", segments=12)
    p.cyl(0.006, 0.006, (0.017, 0, h + 0.023), "Crew orange", segments=10)
    p.cyl(0.008, 0.014, (-0.017, 0, h + 0.012), "Rubber", segments=10)
    p.tube([(-0.017, 0, h + 0.016), (-0.018, 0, h + 0.07), (-0.02, -0.004, h + 0.13)], 0.0045, "Rubber", segments=8)
    return p


PIECES = [
    mark_chit, mark_stack, scrip_bundle, filter_cartridge, filter_case,
    contract_paper, sealed_filing, ledger_book, land_claim, registrar_stamp, forged_seal,
    rebreather, water_canister,
    grain_sack, produce_crate, protein_pack, coffee_tin,
    power_cell, terraformer_part, salvage_scrap, valve, data_core,
    medkit, seed_case, electronics_box, wine_bottle,
    wrench, toolbox, crowbar, flashlight, radio,
]
