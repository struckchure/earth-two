"""The detailed finish: what turns a piece from flat-coloured blocks into
something that sits beside the characters. A piece is built as usual (kit's
helpers, with DETAIL raised so round things are round), and then:

1. Its organic parts (rock, soil, dust, plants) are subdivided and
   displaced: craggy rock, lumpy leaves. Hard parts get a
   proper rounded bevel instead of the 8 mm chamfer, normals hardened so
   flat faces stay flat.
2. It's unwrapped and its look is baked into one colour texture: the
   palette colour, darkened in its crevices (ambient occlusion), the paint
   worn off its edges, grime in patches, and red dust banked up from the
   ground, as much as its part of the world has (Charter Row is kept clean;
   the Fringe is filthy).
3. Its materials are replaced by that one texture: one draw call a piece,
   where it was one per colour.

The toon shader multiplies the texture into the light (shading/toon.go), so
it needs nothing new from the engine. It's slower to build (Cycles bakes
every piece), so it's what make world does, and the contact sheet can skip
it."""
import math
import tempfile
from pathlib import Path

import bmesh
import bpy

import kit

# The biggest a piece's texture gets, in pixels a side: 2048 tripled the
# download (assets/world went to over 100 MB, heavy for the browser build)
# for detail only seen up close.
TEXTURE_MAX = 1024

# Threads each bake may use (0: all of them). make world runs several
# builds at once and shares the CPU out between them.
THREADS = 0

# Segment counts for round things, times kit's: cylinders that read as
# cylinders at arm's length.
DETAIL = 2.5

# Organic materials, and how each is roughened: (how, subdivisions,
# displacement as a share of the part's size). Cloth isn't here: kit's
# cloth() drapes it, and a hard part in a fabric colour stays hard.
ORGANIC = {
    "rock": ({"Rock red", "Rock dark", "Rock pale"}, 3, 0.07),
    "soil": ({"Dust red", "Soil"}, 2, 0.03),
    "leafy": ({"Foliage", "Foliage light", "Crop green", "Veg green"}, 2, 0.04),
}

# Fabrics, dirtied lightly (see _shade).
CLOTH = {"Canvas", "Bleached", "Poly film", "Poly film old"}

# What's painted metal, whose edges wear through to steel.
METAL = ("Hull", "Crew", "Repaint", "Container", "Hazard", "Steel", "Gunmetal", "Rust", "Grating",
         "Charter navy", "Corvane", "Copper", "Brass", "Wrap", "Olive drab")

# How much each part of the world is worn and dusted (0 to 1).
WEAR = {
    "The Hull": (0.7, 0.25), "The Hull market": (0.7, 0.3), "Hull decks": (0.8, 0.25),
    "The Exchange": (0.45, 0.1), "Charter Row": (0.15, 0.05), "The Pads": (0.8, 0.55),
    "Domes and gate": (0.6, 0.5), "The Fringe": (0.9, 0.8), "Items": (0.4, 0.1),
    "Weapons": (0.5, 0.05), "Vehicles": (0.7, 0.55),
}
CLEAN = {"receivership_shuttle", "data_core", "scrip_bundle"}  # Corvane: new and pressed


def organic_kind(material: str) -> str | None:
    for kind, (names, _, _) in ORGANIC.items():
        if material in names:
            return kind
    return None


def _part(name: str, mesh: bpy.types.Mesh, keep) -> bpy.types.Object | None:
    """A copy of mesh with only the faces keep(material name) is true of."""
    bm = bmesh.new()
    bm.from_mesh(mesh)
    names = [m.name for m in mesh.materials]
    drop = [f for f in bm.faces if not keep(names[f.material_index])]
    bmesh.ops.delete(bm, geom=drop, context="FACES")
    if not bm.faces:
        bm.free()
        return None
    m = bpy.data.meshes.new(name)
    bm.to_mesh(m)
    bm.free()
    for mat in mesh.materials:
        m.materials.append(mat)
    o = bpy.data.objects.new(name, m)
    bpy.context.scene.collection.objects.link(o)
    return o


def _roughen(o: bpy.types.Object, kind: str, size: float) -> None:
    """o's faces split finer and pushed in and out along their normals by
    noise: rock cracked and chipped, soil and leaves lumpy. Done here
    rather than with modifiers, so it's the same in every Blender."""
    from mathutils import noise
    bm = bmesh.new()
    bm.from_mesh(o.data)
    target = {"rock": size / 22, "soil": size / 18, "leafy": size / 10}[kind]
    target = max(target, 0.015)
    for _ in range(6):
        long = [e for e in bm.edges if e.calc_length() > target]
        if not long:
            break
        bmesh.ops.subdivide_edges(bm, edges=long, cuts=1, use_grid_fill=True)
    bmesh.ops.triangulate(bm, faces=[f for f in bm.faces if len(f.verts) > 4])
    bm.normal_update()
    _, _, amount = ORGANIC[kind]
    h = amount * size
    for v in bm.verts:
        p = v.co
        if kind == "rock":
            # Broad lumps, and creases where Voronoi cells meet: cracks.
            f = noise.fractal(p * (1.6 / size), 0.6, 2.0, 4)
            d, _ = noise.voronoi(p * (3.0 / size), distance_metric="DISTANCE")
            crack = min(1.0, (d[1] - d[0]) * 3.0)
            off = h * (0.5 * f - 0.6 * (1 - crack) ** 3)
        else:
            off = h * noise.fractal(p * (2.5 / max(size, 0.3)), 0.6, 2.0, 3)
        v.co = p + v.normal * off
    for f in bm.faces:
        f.smooth = kind != "rock"
    bm.to_mesh(o.data)
    bm.free()


def _evaluated(o: bpy.types.Object) -> bmesh.types.BMesh:
    dg = bpy.context.evaluated_depsgraph_get()
    bm = bmesh.new()
    bm.from_mesh(o.evaluated_get(dg).to_mesh())
    return bm


def detail(obj: bpy.types.Object, item: bool) -> None:
    """Organic parts roughened, hard parts bevelled: obj's mesh replaced by
    the result, its modifiers gone."""
    mesh = obj.data
    names = [m.name for m in mesh.materials]
    size = max(obj.dimensions)
    # Sourced models' own materials (not the palette's) are left as they
    # are: they're detailed already.
    hard = _part(obj.name + " hard", mesh, lambda n: organic_kind(n) is None and n in kit.PALETTE)
    # Kept as they are: sourced models' own materials and plain() parts.
    kept = _part(obj.name + " kept", mesh, lambda n: n not in kit.PALETTE)
    parts = [kept] if kept else []
    if hard:
        b = hard.modifiers.new("Bevel", "BEVEL")
        b.width = 0.0025 if item else min(0.02, max(0.006, 0.006 * size))
        b.segments = 3
        b.limit_method = "ANGLE"
        # Above the angle between a round part's facets (so a 7-sided rail
        # isn't bevelled facet by facet), below a box's corners.
        b.angle_limit = math.radians(55)
        b.harden_normals = True
        b.miter_outer = "MITER_ARC"
        parts.append(hard)
    for kind in ORGANIC:
        o = _part(f"{obj.name} {kind}", mesh, lambda n, k=kind: organic_kind(n) == k)
        if o:
            # Each lump of it roughened by its own size, not the piece's.
            dims = o.dimensions
            _roughen(o, kind, max(min(max(dims), 3.0), 0.2))
            parts.append(o)
    out = bmesh.new()
    for o in parts:
        bm = _evaluated(o)
        m = bpy.data.meshes.new("tmp")
        bm.to_mesh(m)
        bm.free()
        out.from_mesh(m)  # appends; material indices are shared (same slot order)
        bpy.data.meshes.remove(m)
        bpy.data.objects.remove(o)
    for mod in list(obj.modifiers):
        obj.modifiers.remove(mod)
    out.to_mesh(mesh)
    out.free()
    del names


# Baking ----------------------------------------------------------------------

def _socket(node, name, kind=None, out=False):
    for s in (node.outputs if out else node.inputs):
        if s.name == name and (kind is None or s.type == kind):
            return s
    raise KeyError(name)


def _rgb(nt, rgb):
    n = nt.nodes.new("ShaderNodeRGB")
    n.outputs[0].default_value = (*rgb[:3], 1)
    return n.outputs[0]


def _mix(nt, fac, a, b, blend="MIX"):
    m = nt.nodes.new("ShaderNodeMix")
    m.data_type = "RGBA"
    m.blend_type = blend
    if isinstance(fac, (int, float)):
        _socket(m, "Factor", "VALUE").default_value = fac
    else:
        nt.links.new(fac, _socket(m, "Factor", "VALUE"))
    for name, v in (("A", a), ("B", b)):
        s = _socket(m, name, "RGBA")
        if isinstance(v, tuple):
            s.default_value = (*v[:3], 1)
        else:
            nt.links.new(v, s)
    return _socket(m, "Result", "RGBA", out=True)


def _scale(nt, col, k):
    """col times k (a number, or a value socket)."""
    if isinstance(k, (int, float)):
        return _mix(nt, 1.0, col, (k, k, k), "MULTIPLY")
    c = nt.nodes.new("ShaderNodeCombineColor")
    for i in range(3):
        nt.links.new(k, c.inputs[i])
    return _mix(nt, 1.0, col, c.outputs[0], "MULTIPLY")


def _math(nt, op, a, b=None, clamp=False):
    n = nt.nodes.new("ShaderNodeMath")
    n.operation = op
    n.use_clamp = clamp
    for i, v in enumerate((a, b)):
        if v is None:
            continue
        if isinstance(v, (int, float)):
            n.inputs[i].default_value = v
        else:
            nt.links.new(v, n.inputs[i])
    return n.outputs[0]


def _range(nt, v, lo, hi, to_lo=0.0, to_hi=1.0):
    n = nt.nodes.new("ShaderNodeMapRange")
    n.clamp = True
    nt.links.new(v, n.inputs["Value"])
    n.inputs["From Min"].default_value = lo
    n.inputs["From Max"].default_value = hi
    n.inputs["To Min"].default_value = to_lo
    n.inputs["To Max"].default_value = to_hi
    return n.outputs["Result"]


def _textured_base(nt):
    """The colour a material's Principled BSDF takes from a texture, if it
    does, read as its raw (display) values like the palette's; and the
    BSDF, unlinked from the output."""
    bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return None, None
    base = bsdf.inputs["Base Color"]
    if not base.is_linked:
        return None, bsdf
    src = base.links[0].from_socket
    for n in nt.nodes:
        if n.type == "TEX_IMAGE" and n.image:
            n.image.colorspace_settings.name = "Non-Color"
    return src, bsdf


def _shade(mat: bpy.types.Material, wear: float, dust: float, size: float, recolour: str | None) -> None:
    """mat turned into the look to bake: an emission of its colour (a
    palette colour, or the texture it already has), aged: worn edges, grime,
    red dust from the ground up, and darkened crevices."""
    nt = mat.node_tree
    colour = kit.colour(mat.get("palette", mat.name))
    textured, bsdf = _textured_base(nt)
    recolour = mat.get("recolour") or recolour  # a sourced part's own
    if textured is None and colour not in kit.PALETTE:
        # A material from elsewhere with a plain colour.
        rgb = tuple(bsdf.inputs["Base Color"].default_value[:3]) if bsdf else (0.5, 0.5, 0.5)
    else:
        rgb = kit.PALETTE.get(colour, (0.5, 0.5, 0.5))
    for link in list(nt.links):
        if link.to_node.type == "OUTPUT_MATERIAL":
            nt.links.remove(link)
    out = next((n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"), None) or nt.nodes.new("ShaderNodeOutputMaterial")
    emit = nt.nodes.new("ShaderNodeEmission")
    nt.links.new(emit.outputs[0], out.inputs["Surface"])
    if textured is None and colour in kit.GLOWING:
        emit.inputs["Color"].default_value = (*rgb, 1)
        return
    coord = nt.nodes.new("ShaderNodeTexCoord")
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    ao = nt.nodes.new("ShaderNodeAmbientOcclusion")
    ao.samples = 8
    ao.inputs["Distance"].default_value = min(0.6, 0.12 * max(size, 0.3))
    occl = _socket(ao, "AO", out=True)
    noise = nt.nodes.new("ShaderNodeTexNoise")
    nt.links.new(coord.outputs["Object"], noise.inputs["Vector"])
    noise.inputs["Scale"].default_value = 2.5 / max(0.3, min(size, 4.0))
    noise.inputs["Detail"].default_value = 8
    noise.inputs["Roughness"].default_value = 0.6
    fine = nt.nodes.new("ShaderNodeTexNoise")
    nt.links.new(coord.outputs["Object"], fine.inputs["Vector"])
    fine.inputs["Scale"].default_value = 18 / max(0.3, min(size, 4.0))
    fine.inputs["Detail"].default_value = 4

    if textured is not None:
        base = textured
        if recolour:
            # Repainted: the palette colour, keeping the texture's light and
            # dark (its wear, dents and dirt).
            bw = nt.nodes.new("ShaderNodeRGBToBW")
            nt.links.new(textured, bw.inputs[0])
            shade = _range(nt, bw.outputs[0], 0.0, 0.6, 0.25, 1.25)
            base = _mix(nt, 0.85, textured, _scale(nt, _rgb(nt, kit.PALETTE[recolour]), shade))
        col = base
        edges = 0.0  # a sourced texture is worn already
    else:
        col = _mix(nt, _range(nt, fine.outputs["Fac"], 0.3, 0.7), tuple(c * 0.92 for c in rgb), tuple(min(1, c * 1.08) for c in rgb))
        edges = wear
    if edges:
        bevel = nt.nodes.new("ShaderNodeBevel")
        bevel.samples = 4
        bevel.inputs["Radius"].default_value = min(0.03, 0.01 * max(size, 0.3))
        dot = nt.nodes.new("ShaderNodeVectorMath")
        dot.operation = "DOT_PRODUCT"
        nt.links.new(bevel.outputs["Normal"], dot.inputs[0])
        nt.links.new(geo.outputs["Normal"], dot.inputs[1])
        edge = _range(nt, dot.outputs["Value"], 0.995, 0.93)
        chipped = _math(nt, "MULTIPLY", edge, _range(nt, noise.outputs["Fac"], 0.35, 0.6), clamp=True)
        metal = any(m in colour for m in METAL)
        worn = _rgb(nt, (0.42, 0.41, 0.39)) if metal else _scale(nt, col, 1.35)
        col = _mix(nt, _math(nt, "MULTIPLY", chipped, edges * (0.9 if metal else 0.5)), col, worn)
    # Cloth takes dirt finer and fainter than metal: patches the size of
    # metal's read as camouflage on a curtain.
    cloth = colour.startswith("Fabric") or colour in CLOTH
    if cloth:
        wear, dust = wear * 0.35, dust * 0.5
    # Grime in patches, heavier where it's occluded.
    grime = _math(nt, "MULTIPLY", _range(nt, noise.outputs["Fac"], 0.45, 0.75), _range(nt, occl, 1.0, 0.4, 0.35, 1.0), clamp=True)
    col = _mix(nt, _math(nt, "MULTIPLY", grime, 0.3 * wear), col, _scale(nt, col, 0.55))
    # Dust banked up from the ground, and settled on what faces up.
    z = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Position"], z.inputs[0])
    low = _range(nt, z.outputs["Z"], min(0.9, 0.35 * max(size, 0.5)), 0.0)
    up = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Normal"], up.inputs[0])
    top = _range(nt, up.outputs["Z"], 0.6, 1.0, 0.0, 0.5)
    dusty = _math(nt, "MULTIPLY", _math(nt, "MAXIMUM", low, top), _range(nt, fine.outputs["Fac"], 0.3, 0.65), clamp=True)
    col = _mix(nt, _math(nt, "MULTIPLY", dusty, 0.4 * dust), col, kit.PALETTE["Dust red"])
    # Ambient occlusion last: crevices darkened, as painted shadow.
    col = _mix(nt, _range(nt, occl, 0.2, 1.0, 0.4, 0.0), col, _scale(nt, col, 0.3))
    nt.links.new(col, emit.inputs["Color"])


def _unwrap(obj: bpy.types.Object) -> None:
    """A fresh UV map for the baked texture, made the active one (a sourced
    model keeps its own for reading its textures)."""
    mesh = obj.data
    uv = mesh.uv_layers.new(name="Baked")
    mesh.uv_layers.active = uv
    bpy.context.view_layer.update()
    for o in bpy.context.view_layer.objects:
        o.select_set(o == obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    # Islands weighted by their area, so a decimated model's slivers don't
    # crowd out its big faces.
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.004, area_weight=1.0, scale_to_bounds=False)
    bpy.ops.uv.pack_islands(margin=0.004)
    bpy.ops.object.mode_set(mode="OBJECT")


def bake(obj: bpy.types.Object, category: str, name: str, high: bpy.types.Object | None = None,
         recolour: str | None = None) -> None:
    """obj unwrapped, its look baked into one texture, and that texture its
    only material. With high, the look is high's (a more detailed version of
    the same thing, its crevices and all) baked onto obj."""
    shading = high or obj
    size = max(obj.dimensions)
    area = sum(p.area for p in obj.data.polygons)
    # Texels: about 220 a metre across the piece, a power of two; twice that
    # on a low-poly stand-in, whose texture is all its detail.
    density = 440 if high is not None and obj.get("lowpoly") else 220
    cap = max(TEXTURE_MAX, int(obj.get("texture", 0)))
    px = 2 ** round(math.log2(max(256, min(cap, math.sqrt(area) * density))))
    if "Baked" not in obj.data.uv_layers:
        _unwrap(obj)
    else:
        obj.data.uv_layers.active = obj.data.uv_layers["Baked"]
    image = bpy.data.images.new(f"{name} albedo", px, px, alpha=False)
    # The game draws a texture's values as they are, as it does the flat
    # colours' values: so the texture keeps them unconverted.
    image.colorspace_settings.name = "Non-Color"
    wear, dust = WEAR.get(category, (0.5, 0.2))
    if name in CLEAN:
        wear, dust = 0.1, 0.05
    # Materials are shared (the palette's by every piece): each piece bakes
    # from its own copies.
    for i, mat in enumerate(list(shading.data.materials)):
        if mat is None:
            # An empty slot (a join can leave one): plain alloy.
            mat = kit.material("Hull alloy")
        own = mat.copy()
        own["palette"] = mat.get("palette", mat.name)
        shading.data.materials[i] = own
        _shade(own, wear, dust, size, recolour)
    # The target: an image node, active, in every material of what's baked to.
    if high:
        target_mat = bpy.data.materials.new(f"{name} target")
        target_mat.use_nodes = True
        obj.data.materials.clear()
        obj.data.materials.append(target_mat)
    for mat in obj.data.materials:
        node = mat.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = image
        mat.node_tree.nodes.active = node
    s = bpy.context.scene
    engine = s.render.engine
    s.render.engine = "CYCLES"
    s.cycles.device = "CPU"
    s.cycles.samples = 12
    if THREADS:
        s.render.threads_mode = "FIXED"
        s.render.threads = THREADS
    s.render.bake.margin = 16
    s.render.bake.margin_type = "EXTEND"
    for o in bpy.context.view_layer.objects:
        o.select_set(o in (obj, high))
    bpy.context.view_layer.objects.active = obj
    if high:
        s.render.bake.use_selected_to_active = True
        # Out past any decal (they're 4–10 mm proud), back in past the detail.
        s.render.bake.cage_extrusion = max(0.015, 0.012 * size)
        s.render.bake.max_ray_distance = max(0.06, 0.08 * size)
    else:
        s.render.bake.use_selected_to_active = False
    # Every piece is built standing on the origin, so the pieces built
    # before this one are all still there, in the same place: hidden from
    # the bake, or their shapes shade this one (ambient occlusion). The
    # target, too, sits on the very surface being shaded.
    hidden = [o for o in s.objects if o not in (obj, high) and not o.hide_render]
    for o in hidden:
        o.hide_render = True
    rays = ("visible_diffuse", "visible_glossy", "visible_shadow", "visible_transmission", "visible_volume_scatter")
    if high is not None:
        kept = {r: getattr(obj, r) for r in rays}
        for r in rays:
            setattr(obj, r, False)
    bpy.ops.object.bake(type="EMIT")
    if high is not None:
        for r, v in kept.items():
            setattr(obj, r, v)
    for o in hidden:
        o.hide_render = False
    s.render.engine = engine
    s.render.bake.use_selected_to_active = False

    path = Path(tempfile.mkdtemp()) / f"{name}.png"
    image.filepath_raw = str(path)
    image.file_format = "PNG"
    image.save()
    image.reload()
    image.pack()
    final = bpy.data.materials.new(name)
    final.use_nodes = True
    bsdf = final.node_tree.nodes["Principled BSDF"]
    tex = final.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    final.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.8
    final.diffuse_color = (1, 1, 1, 1)
    used = list(obj.data.materials) + (list(high.data.materials) if high else [])
    obj.data.materials.clear()
    obj.data.materials.append(final)
    for p in obj.data.polygons:
        p.material_index = 0
    # Only the baked UV map goes to the game.
    # (By name, one at a time: a removal shifts the others' references.)
    for name in [u.name for u in obj.data.uv_layers if u.name != "Baked"]:
        obj.data.uv_layers.remove(obj.data.uv_layers[name])
    for m in set(used):
        if m.users == 0:
            bpy.data.materials.remove(m)
    _split(obj, final)


# raylib indexes a mesh's vertices with 16 bits: more than 65,535 in one
# mesh and its triangles come out garbage. Exported, a mesh has at most a
# vertex a face corner, so a piece with more corners than this is split.
SPLIT = 60000


def _split(obj: bpy.types.Object, final: bpy.types.Material) -> None:
    """A big piece cut into chunks (along its longest side), each under the
    limit and each with its own material slot: the glTF exporter writes a
    mesh a slot, and raylib loads each as a mesh of its own. They all share
    the one texture."""
    mesh = obj.data
    corners = len(mesh.loops)
    if corners <= SPLIT:
        return
    axis = max(range(3), key=lambda i: obj.dimensions[i])
    faces = sorted(mesh.polygons, key=lambda p: p.center[axis])
    chunks = math.ceil(corners / (SPLIT * 0.9))
    per = corners / chunks
    for k in range(1, chunks):
        mesh.materials.append(final.copy())
    done = 0
    for p in faces:
        p.material_index = min(chunks - 1, int(done // per))
        done += p.loop_total


def prepare() -> None:
    """kit set up for the detailed build: rounder round things, and decals
    kept apart to be baked."""
    kit.DETAIL = DETAIL
    kit.FINISHING = True


def _with_decals(obj: bpy.types.Object, decals: bpy.types.Object | None) -> bpy.types.Object:
    """A copy of obj with its decals joined on: what's baked from."""
    high = obj.copy()
    high.data = obj.data.copy()
    bpy.context.scene.collection.objects.link(high)
    if decals:
        bpy.context.view_layer.update()
        for o in bpy.context.view_layer.objects:
            o.select_set(o in (high, decals))
        bpy.context.view_layer.objects.active = high
        bpy.ops.object.join()
    return high


def finish(obj: bpy.types.Object, category: str, kind: str) -> None:
    """The detailed finish for a built piece: a sourced one (see
    polyhaven.py) is decimated to its budget if it's over, and baked from its
    full detail; a made one is roughened and bevelled, then baked. Either way
    its decals are baked in from just above it."""
    recolour = obj.get("recolour") or None
    decals = bpy.data.objects.get(obj.get("decals", ""))
    # Always baked from a copy of itself (with its decals): where two parts'
    # faces share a plane, both then take their colour along the same ray,
    # so they can't flicker between two colours in the game.
    if obj.get("detail"):
        # A low-poly stand-in: its detailed model, roughened and bevelled,
        # is what's baked onto it.
        model = bpy.data.objects[obj["detail"]]
        detail(model, kind == "item")
        high = _with_decals(model, decals)
        bpy.data.objects.remove(model)
        bpy.context.view_layer.update()
    elif obj.get("source"):
        budget = int(obj.get("budget", 0))
        tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
        over = budget and tris > budget
        high = _with_decals(obj, decals)
        if over:
            dec = obj.modifiers.new("Decimate", "DECIMATE")
            dec.ratio = budget / tris
            dg = bpy.context.evaluated_depsgraph_get()
            mesh = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
            obj.modifiers.remove(dec)
            obj.data = mesh
    else:
        detail(obj, kind == "item")
        high = _with_decals(obj, decals)
    bake(obj, category, obj.name, high=high, recolour=recolour)
    bpy.data.objects.remove(high)
    bpy.context.view_layer.update()
