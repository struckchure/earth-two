"""Builds Earth Two's wardrobe: clothes, hair, glasses and skins for the
people tools/makehuman/people.py makes.

Run it in Blender with MPFB installed and the MakeHuman asset packs loaded
(see the README), after people.py has built the bodies into BODIES_DIR:

    blender -b --python tools/makehuman/wardrobe.py -- BODIES_DIR

For each person in people.PEOPLE it fits every item in CATALOGUE to the same
body, rigs it to the same game engine rig, and exports it on its own to
BODIES_DIR/<person>/<slot>/<item>.glb (the rig, no clips: the game poses it
with the body's). It writes the skins to BODIES_DIR/skins/ and lists
everything in BODIES_DIR/wardrobe.json.

Clothes are layered: each is moved out to sit just over what it's worn
over wherever the two meet (the skin and the underwear, and for tops every
bottom too), so what's underneath can't poke through it.

Where the body bends, skin can still poke through clothes as they move, so
an item hides the skin it covers: the body comes in regions (people.REGIONS)
and an item hides the regions it covers nearly all of, bringing back as a
skin patch, a mesh in its own file, the little of each it doesn't cover.
The game draws the patches in the character's skin tone. Underwear on a
region stays until an item covers the underwear.
"""

import json
import os
import struct
import sys

import bpy
import numpy
from mathutils.bvhtree import BVHTree

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import people  # noqa: E402  (after the path: people.py sits next to this file)

from bl_ext.user_default.mpfb.services import HumanService  # noqa: E402

# What each person can wear: slot -> [(item, display name)]. Items are
# MakeHuman assets (system assets and the shirts01, pants01 and glasses01
# packs); "hair" items are in MPFB's hair folder, the rest in clothes. An
# outfit is a one-piece top and bottom: wearing one takes off the top and
# bottom, and the other way round.
CATALOGUE = {
    "man": {
        "hair": [("short02", "Short"), ("short04", "Crop"), ("short01", "Side part"),
                 ("afro01", "Afro"), ("braid01", "Braids")],
        "glasses": [("frankyaye_glasses_library_male", "Library"), ("kwnet_at_optical_glasses", "Optical"),
                    ("toigo_round_glasses_leopard", "Round")],
        "top": [("joepal_crude_t-shirt_female", "T-shirt"), ("namuhekam_male_polo_shirt", "Polo"),
                ("toigo_fisherman_sweater", "Sweater")],
        "bottom": [("cortu_cargo_pants", "Cargo pants"), ("toigo_wool_pants", "Trousers"),
                   ("cortu_jeans_shorts", "Jeans shorts")],
        "outfit": [("male_casualsuit01", "Denim shirt & jeans"), ("male_casualsuit03", "Striped shirt & jeans"),
                   ("male_casualsuit05", "Jacket & jeans"), ("male_casualsuit06", "White tee & jeans"),
                   ("male_worksuit01", "Overalls"), ("male_elegantsuit01", "Suit & tie")],
        "shoes": [("shoes05", "White trainers"), ("shoes06", "Blue trainers"), ("shoes02", "Grey sneakers"),
                  ("shoes01", "Brown brogues"), ("shoes04", "Black shoes"), ("shoes03", "Boots")],
    },
    "woman": {
        "hair": [("bob02", "Bob"), ("ponytail01", "Ponytail"), ("long01", "Long"),
                 ("afro01", "Afro"), ("braid01", "Braids"), ("short03", "Pixie")],
        "glasses": [("kwnet_at_optical_glasses", "Optical"), ("toigo_round_glasses_leopard", "Round"),
                    ("spamrakuen_sagerfrogs_glasses_02", "Frames")],
        "top": [("joepal_crude_t-shirt_female", "T-shirt"), ("toigo_keyhole_tank_top", "Tank top"),
                ("toigo_camisole_top", "Camisole"), ("toigo_fisherman_sweater", "Sweater")],
        "bottom": [("cortu_cargo_pants", "Cargo pants"), ("toigo_harem_pants", "Harem pants"),
                   ("cortu_jeans_shorts", "Jeans shorts")],
        "outfit": [("female_casualsuit01", "Tee & jeans"), ("female_casualsuit02", "Tee & shorts"),
                   ("female_sportsuit01", "Sportswear"), ("female_elegantsuit01", "Blouse & skirt")],
        "shoes": [("shoes05", "White trainers"), ("shoes06", "Blue trainers"), ("shoes04", "Black shoes"),
                  ("shoes03", "Boots")],
    },
}

# The skins on offer, by MakeHuman skin name without the _male/_female.
SKINS = [("young_african", "Dark"), ("middleage_african", "Dark, older"),
         ("young_asian", "Light brown"), ("middleage_asian", "Light brown, older"),
         ("young_caucasian", "Fair"), ("middleage_caucasian", "Fair, older")]

# An item hides a region's skin when it covers at least HIDES of it, and
# the underwear on a region when it covers at least COVERED of the
# underwear itself (whose inside faces never count as covered, so this is
# nearly all of it). Otherwise the underwear stays: clothes are layered over
# it, and it fills any opening they have.
HIDES = 0.97
COVERED = 0.95
# How far out from the skin an item can sit and still count as over it.
# Rays start a little under the skin: MakeHuman clothes fit so tight they
# can sit closer to it than any gap left above it.
REACH = 0.08
UNDER = 0.01
# Skin patches are named SKIN_PREFIX + region in an item's file. They reach
# PATCH_RINGS rings of faces in under the item's edge, and their skin under
# the item is tucked TUCK in, below the item.
SKIN_PREFIX = "skin."
PATCH_RINGS = 2
TUCK = 0.004
# Layering (see layer_over): an item is kept at least LAYER_GAP out from the
# skin and underwear, and CLOTHES_GAP out from other clothes it's worn over
# (they bend apart more as the body moves), where those are within
# LAYER_REACH.
LAYER_GAP = 0.006
CLOTHES_GAP = 0.012
LAYER_REACH = 0.04
# What each slot is worn over, besides the skin and underwear: tops go over
# bottoms.
WORN_OVER = {"top": ["bottom"], "bottom": [], "outfit": [], "shoes": []}

TEXTURE_SIZE = {"hair": 512, "top": 1024, "bottom": 1024, "outfit": 1024, "glasses": 512, "shoes": 512}
# Items whose textures need their alpha (cutout hair cards, glasses frames,
# a ragged hem; see uses_alpha) stay PNG; the rest export as JPEG, several
# times smaller.
KEEP_ALPHA = {"hair", "glasses"}
# An item's alpha counts when more than SEE_THROUGH of its corners are see-through.
SEE_THROUGH = 0.02


class Body:
    """The bare body, prepared like people.split_body prepares it, with what
    covering needs: each vertex's place and normal, and each region's skin
    faces (not MakeHuman's helper geometry, which the game never draws)."""

    def __init__(self, basemesh):
        people.prepare_body(basemesh)
        self.obj = basemesh
        mesh = basemesh.data
        world = basemesh.matrix_world
        normals = world.to_3x3().inverted().transposed()
        self.verts = [(world @ v.co, (normals @ v.normal).normalized()) for v in mesh.vertices]
        body = basemesh.vertex_groups["body"].index
        skin = [any(g.group == body and g.weight > 0.5 for g in v.groups) for v in mesh.vertices]
        self.region_faces = {}
        for poly, region in zip(mesh.polygons, people.face_regions(basemesh)):
            if all(skin[v] for v in poly.vertices):
                self.region_faces.setdefault(region, []).append(poly.index)
        self.region_verts = {r: sorted({v for f in faces for v in mesh.polygons[f].vertices})
                             for r, faces in self.region_faces.items()}
        self.vert_regions = {}
        for r, vs in self.region_verts.items():
            for v in vs:
                self.vert_regions.setdefault(v, set()).add(r)

    def skin(self):
        """The skin's vertices (world space) and faces."""
        polys = self.obj.data.polygons
        return [co for co, _ in self.verts], [polys[f].vertices[:] for fs in self.region_faces.values() for f in fs]

    def covered(self, obj):
        """Which skin vertices obj is over, found by casting from under the
        skin outward, and the share of each region's."""
        tree = BVHTree.FromObject(obj, bpy.context.evaluated_depsgraph_get())
        over = {}
        for vs in self.region_verts.values():
            for v in vs:
                if v not in over:
                    co, normal = self.verts[v]
                    over[v] = tree.ray_cast(co - normal * UNDER, normal, REACH + UNDER)[0] is not None
        share = {r: sum(over[v] for v in vs) / len(vs) for r, vs in self.region_verts.items()}
        return over, share

    def patch(self, region, over, hidden):
        """A copy of region's skin with only the faces an item leaves (some
        of) uncovered, rigged like the body, or None if it covers them all.
        hidden are the regions the item hides."""
        polys = self.obj.data.polygons
        faces = self.region_faces[region]
        open_ = {f for f in faces if not all(over[v] for v in polys[f].vertices)}
        if not open_:
            return None
        # Rings of faces more, tucked under the item's edge, so no sliver of
        # the hidden region shows where the edge sits at an angle or away
        # from the skin.
        keep = open_
        for _ in range(PATCH_RINGS):
            edge = {v for f in keep for v in polys[f].vertices}
            keep = {f for f in faces if any(v in edge for v in polys[f].vertices)}
        # What's under the item goes in a little, so it can't show through
        # where the item fits tight; the open faces' own corners, and any
        # corner shared with a region that stays, keep to the skin.
        shown = {v for f in open_ for v in polys[f].vertices}
        tuck = {v for f in keep for v in polys[f].vertices
                if over[v] and v not in shown and self.vert_regions[v] <= hidden}
        patch = self.obj.copy()
        patch.data = self.obj.data.copy()
        patch.name = SKIN_PREFIX + region
        bpy.context.collection.objects.link(patch)
        for mod in list(patch.modifiers):
            if mod.type != "ARMATURE":
                patch.modifiers.remove(mod)
        for v in tuck:
            vert = patch.data.vertices[v]
            vert.co -= vert.normal * TUCK
        for poly in patch.data.polygons:
            poly.select = poly.index not in keep
        bpy.ops.object.select_all(action="DESELECT")
        bpy.context.view_layer.objects.active = patch
        patch.select_set(True)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.delete(type="FACE")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.delete_loose()
        bpy.ops.object.mode_set(mode="OBJECT")
        # The game draws it in the skin tone, so the material only needs to
        # exist: no texture to carry.
        patch.data.materials.clear()
        patch.data.materials.append(skin_material())
        return patch


def skin_material():
    mat = bpy.data.materials.get("skin")
    if mat is None:
        mat = bpy.data.materials.new("skin")
        mat.use_nodes = True
        # White: the game multiplies the skin tone texture by it.
        bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
        bsdf.inputs["Base Color"].default_value = (1, 1, 1, 1)
    return mat


def obj_vertex_count(path):
    with open(path) as f:
        return sum(1 for line in f if line.startswith("v "))


def fit(slot, item, basemesh):
    kind = "hair" if slot == "hair" else "clothes"
    mhclo = people.asset(kind, item, ".mhclo")
    obj_file = os.path.join(os.path.dirname(mhclo), next(
        line.split()[1] for line in open(mhclo) if line.startswith("obj_file")))
    # Smooth the coarse ones; the rest are detailed enough as they are.
    subdiv = 1 if obj_vertex_count(obj_file) < 1500 else 0
    obj = HumanService.add_mhclo_asset(mhclo, basemesh, asset_type="Hair" if slot == "hair" else "Clothes",
                                       subdiv_levels=subdiv, material_type="GAMEENGINE")
    # MPFB subdivides for renders only; the export takes the viewport's.
    for mod in obj.modifiers:
        if mod.type == "SUBSURF":
            mod.levels = mod.render_levels
    people.shrink_textures(obj, TEXTURE_SIZE[slot])
    return obj


def underwear_covered(obj, underwear):
    """The share of each region's underwear obj is over, found like
    Body.covered finds the skin's."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    tree = BVHTree.FromObject(obj, depsgraph)
    over = {}
    for u in underwear:
        region = u.name[len(people.REGION_PREFIX):].split(".")[0]
        ev = u.evaluated_get(depsgraph)
        mesh = ev.to_mesh()
        normals = u.matrix_world.to_3x3().inverted().transposed()
        for v in mesh.vertices:
            co, n = u.matrix_world @ v.co, (normals @ v.normal).normalized()
            over.setdefault(region, []).append(tree.ray_cast(co - n * UNDER, n, REACH + UNDER)[0] is not None)
        ev.to_mesh_clear()
    return {r: sum(hits) / len(hits) for r, hits in over.items()}


def surface(objs, extra=()):
    """A BVH tree of objs as they are (modifiers applied), in world space,
    and the extra (verts, polys)."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    verts, polys = [], []
    for o in objs:
        ev = o.evaluated_get(depsgraph)
        mesh = ev.to_mesh()
        base = len(verts)
        verts.extend(o.matrix_world @ v.co for v in mesh.vertices)
        polys.extend([base + i for i in p.vertices] for p in mesh.polygons)
        ev.to_mesh_clear()
    for vs, ps in extra:
        base = len(verts)
        verts.extend(vs)
        polys.extend([base + i for i in p] for p in ps)
    return BVHTree.FromPolygons(verts, polys)


def layer_over(obj, inner, gap, extra=()):
    """Moves obj out wherever it's under, in, or within gap of the inner
    objects (what it's worn over) and the extra (verts, polys), to gap above
    the outermost of them, looking along each vertex's normal. obj's
    smoothing is applied first, so it's the final surface that moves."""
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    if obj.data.shape_keys:
        bpy.ops.object.shape_key_remove(all=True, apply_mix=True)
    for mod in list(obj.modifiers):
        if mod.type == "SUBSURF":
            bpy.ops.object.modifier_apply(modifier=mod.name)
    tree = surface(inner, extra)
    world = obj.matrix_world
    back = world.inverted()
    normals = world.to_3x3().inverted().transposed()
    moved = 0
    for v in obj.data.vertices:
        co, n = world @ v.co, (normals @ v.normal).normalized()
        start = co - n * LAYER_REACH
        top = outermost(tree, start, n, LAYER_REACH + gap)
        if top is not None and top + gap > LAYER_REACH:
            v.co = back @ (start + n * (top + gap))
            moved += 1
    obj.data.update()
    print("LAYER", obj.name, "moved", moved, "of", len(obj.data.vertices))


def outermost(tree, start, direction, length):
    """How far along the ray from start the last surface of tree within
    length is, or None."""
    last, t = None, 0.0
    while t < length:
        hit, _, _, dist = tree.ray_cast(start + direction * t, direction, length - t)
        if hit is None:
            break
        t += dist + 1e-5
        last = t
    return last


def uses_alpha(obj):
    """Whether obj's materials cut anything out: an alpha-linked texture
    that's see-through at more than a little of the mesh (sampled at its
    texture coordinates; textures are often clear where no face uses them)."""
    layer = obj.data.uv_layers.active
    if layer is None:
        return False
    uv = numpy.empty(2 * len(layer.data), dtype=numpy.float32)
    layer.data.foreach_get("uv", uv)
    uv = uv.reshape(-1, 2) % 1.0
    for slot in obj.material_slots:
        tree = slot.material.node_tree if slot.material else None
        if tree is None:
            continue
        for link in tree.links:
            if link.to_node.type == "BSDF_PRINCIPLED" and link.to_socket.name == "Alpha" \
                    and link.from_node.type == "TEX_IMAGE" and link.from_node.image:
                image = link.from_node.image
                w, h = image.size
                pixels = numpy.empty(w * h * 4, dtype=numpy.float32)
                image.pixels.foreach_get(pixels)
                alpha = pixels[3::4].reshape(h, w)
                x = numpy.minimum((uv[:, 0] * w).astype(int), w - 1)
                y = numpy.minimum((uv[:, 1] * h).astype(int), h - 1)
                if (alpha[y, x] < 0.5).mean() > SEE_THROUGH:
                    return True
    return False


def base_colour_only(obj, keep_alpha):
    """Unhooks everything but the base colour (and, if keep_alpha, the
    alpha) from obj's materials: the game only draws the base colour, and a
    texture whose alpha is in use exports as PNG rather than JPEG."""
    for slot in obj.material_slots:
        tree = slot.material.node_tree if slot.material else None
        if tree is None:
            continue
        for link in list(tree.links):
            socket = link.to_socket.name
            if link.to_node.type == "BSDF_PRINCIPLED" and socket != "Base Color" and not (keep_alpha and socket == "Alpha"):
                tree.links.remove(link)


def export_item(obj, patches, rig, path, keep_alpha):
    bpy.ops.object.select_all(action="DESELECT")
    for o in [obj, rig] + patches:
        o.select_set(True)
    base_colour_only(obj, keep_alpha)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_morph=False,
        export_animations=False,
        export_image_format="AUTO" if keep_alpha else "JPEG",
        export_jpeg_quality=85,
    )


def export_skin(name, out_dir):
    """Writes MakeHuman skin name's diffuse texture, at 1024, as a JPEG."""
    mhmat = people.asset("skins", name, ".mhmat")
    texture = next(line.split(None, 1)[1].strip() for line in open(mhmat) if line.startswith("diffuseTexture"))
    image = bpy.data.images.load(os.path.join(os.path.dirname(mhmat), texture))
    image.scale(1024, 1024)
    scene = bpy.context.scene
    scene.render.image_settings.file_format = "JPEG"
    scene.render.image_settings.quality = 85
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".jpg")
    image.save_render(path, scene=scene)
    bpy.data.images.remove(image)
    return path


def glb_json(path):
    with open(path, "rb") as f:
        data = f.read()
    length = struct.unpack_from("<I", data, 12)[0]
    return json.loads(data[20:20 + length])


def mesh_indices(glb):
    """The index raylib gives each node's meshes: it loads every primitive of
    every mesh node, in node order."""
    out, i = {}, 0
    for node in glb["nodes"]:
        if "mesh" in node:
            n = len(glb["meshes"][node["mesh"]]["primitives"])
            out[node["name"]] = list(range(i, i + n))
            i += n
    return out


def joint_names(glb):
    return [glb["nodes"][j]["name"] for j in glb["skins"][0]["joints"]]


def main():
    out = sys.argv[sys.argv.index("--") + 1]
    wardrobe = {"bodies": []}
    for name, (gender, skin, underclothes) in people.PEOPLE.items():
        body_glb = glb_json(os.path.join(out, name + ".glb"))
        body_joints = joint_names(body_glb)
        indices = mesh_indices(body_glb)
        regions, underwear, skin_meshes = {}, {}, []
        for node, idx in indices.items():
            if node.startswith(people.REGION_PREFIX):
                parts = node[len(people.REGION_PREFIX):].split(".")
                if len(parts) == 1:  # the body itself
                    regions.setdefault(parts[0], []).extend(idx)
                    skin_meshes.extend(idx)
                else:  # underwear on the region
                    underwear.setdefault(parts[0], []).extend(idx)

        people.clear_scene()
        basemesh, rig = people.make_person(gender, skin, underclothes)
        body = Body(basemesh)
        underwear_objs = [o for o in bpy.data.objects if o.name.startswith(people.REGION_PREFIX)]

        sex = "male" if gender > 0.5 else "female"
        skins = []
        for base, label in SKINS:
            path = export_skin(base + "_" + sex, os.path.join(out, "skins"))
            skins.append({"name": label, "path": os.path.relpath(path, os.path.dirname(out))})

        # Bottoms first: tops are layered over them, so they stay in the
        # scene until the body's done.
        order = sorted(CATALOGUE[name].items(), key=lambda kv: kv[0] != "bottom")
        kept = {}
        slots = {}
        for slot, items in order:
            for item, label in items:
                obj = fit(slot, item, basemesh)
                hides, covers, patches = [], [], []
                if slot in WORN_OVER:
                    layer_over(obj, underwear_objs, LAYER_GAP, [body.skin()])
                    clothes = [o for s in WORN_OVER[slot] for o in kept.get(s, [])]
                    if clothes:
                        layer_over(obj, clothes, CLOTHES_GAP)
                if slot not in ("hair", "glasses"):
                    over, share = body.covered(obj)
                    hides = sorted(r for r, x in share.items() if x >= HIDES)
                    under = underwear_covered(obj, underwear_objs)
                    covers = sorted(r for r, x in under.items() if x >= COVERED)
                    patches = [p for p in (body.patch(r, over, set(hides)) for r in hides) if p]
                path = os.path.join(out, name, slot, item + ".glb")
                keep_alpha = slot in KEEP_ALPHA or uses_alpha(obj)
                if keep_alpha and slot not in KEEP_ALPHA:
                    print("ALPHA", name, item)
                export_item(obj, patches, rig, path, keep_alpha)
                glb = glb_json(path)
                if joint_names(glb) != body_joints:
                    raise SystemExit("%s: %s's joints don't match the body's" % (name, item))
                skin = [i for node, idx in mesh_indices(glb).items() if node.startswith(SKIN_PREFIX) for i in idx]
                slots.setdefault(slot, []).append({
                    "name": label, "path": os.path.relpath(path, os.path.dirname(out)),
                    "hides": hides, "covers": covers, "skinMeshes": skin})
                print("ITEM", name, slot, item, "hides", hides, "covers", covers,
                      "%.0f KB" % (os.path.getsize(path) / 1024))
                for o in patches:
                    bpy.data.objects.remove(o, do_unlink=True)
                if slot == "bottom":
                    kept.setdefault(slot, []).append(obj)
                else:
                    bpy.data.objects.remove(obj, do_unlink=True)
                bpy.data.orphans_purge(do_recursive=True)
        for objs in kept.values():
            for o in objs:
                bpy.data.objects.remove(o, do_unlink=True)
        slots = {slot: slots[slot] for slot in CATALOGUE[name] if slot in slots}

        wardrobe["bodies"].append({
            "name": name,
            "model": os.path.relpath(os.path.join(out, name + ".glb"), os.path.dirname(out)),
            "skinMeshes": skin_meshes,
            "regions": regions,
            "underwear": underwear,
            "skins": skins,
            "slots": slots,
        })
    with open(os.path.join(out, "wardrobe.json"), "w") as f:
        json.dump(wardrobe, f, indent=1)
    print("WARDROBE", os.path.join(out, "wardrobe.json"))


if __name__ == "__main__":
    main()
