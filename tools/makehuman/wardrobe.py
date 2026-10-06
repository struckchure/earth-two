"""Builds Earth Two's wardrobe: clothes, hair, glasses, masks and skins for the
people tools/makehuman/people.py makes, and the factions' looks.

Run it in Blender with MPFB installed and the MakeHuman asset packs loaded
(see the README), after people.py has built the bodies into BODIES_DIR:

    blender -b --python tools/makehuman/wardrobe.py -- BODIES_DIR

For each person in cast.PEOPLE it fits every item in cast.CATALOGUE to the same
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

import bisect
import json
import math
import os
import struct
import sys

import bpy
import numpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import people  # noqa: E402  (after the path: people.py sits next to this file)
from cast import CATALOGUE, FACE, FACES, LOOKS, PEOPLE, SKINS  # noqa: E402

from bl_ext.user_default.mpfb.services import HumanService  # noqa: E402

# How far a face's seam with the rest of the body may be from the body's.
SEAM = 0.0005

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
# How far, in rings of edges, layering spreads each push (widening it by
# LAYER_SPREAD, then smoothing it over LAYER_SMOOTH): cloth drapes over what's
# under it, so a shirt over a waistband bulges smoothly instead of taking
# the shape of every belt loop and fold.
LAYER_SPREAD = 2
LAYER_SMOOTH = 6
# Draping (see drape): what's worn on the trunk hangs from where it rests,
# the chest and shoulder blades, down to the hips, across the hollows of the
# waist and the small of the back rather than into them. DRAPED is the
# body regions each slot drapes over, DRAPE_BINS how many ways round the body
# and DRAPE_STEP how often up it it's worked out, and DRAPE_MOST the furthest
# it moves cloth out.
DRAPED = {"top": {"torso", "hips"}, "outfit": {"torso"}, "coat": {"torso", "hips"}}
DRAPE_BINS = 72
DRAPE_STEP = 0.01
DRAPE_MOST = 0.12
# Draped cloth sits over the hull as far as it sat over the body (at least
# DRAPE_GAP), smoothed over DRAPE_LIFT_SMOOTH rings of edges; cloth further
# than DRAPE_NEAR out from the trunk (sleeves) isn't draped.
DRAPE_GAP = 0.008
DRAPE_LIFT_SMOOTH = 20
DRAPE_NEAR = 0.06
# Cloth drapes from ARMPIT below the shoulder joints down: above, it's held
# by the shoulders as it's made, and at the sides it hangs from under the
# arms, not off the shoulder over them. It eases in over DRAPE_FADE.
ARMPIT = 0.05
DRAPE_FADE = 0.06
# What each slot is worn over, besides the skin and underwear: tops go over
# bottoms.
WORN_OVER = {"top": ["bottom"], "bottom": [], "outfit": [], "coat": ["bottom"], "shoes": []}

TEXTURE_SIZE = {"hair": 512, "top": 1024, "bottom": 1024, "outfit": 1024, "coat": 1024, "glasses": 512, "mask": 512,
                "shoes": 512}
# Worn over the face or head, on top of the skin: they hide none of it.
OVER_FACE = ("hair", "glasses", "mask")
# A repainted item keeps this much of its texture's own colour.
RECOLOUR_KEEP = 0.12
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
        # Normals first: moving a vertex makes Blender recompute them from
        # the half-moved mesh.
        verts = patch.data.vertices
        normals = {v: verts[v].normal.copy() for v in tuck}
        for v in tuck:
            verts[v].co -= normals[v] * TUCK
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


def base_colour_node(tree):
    """The image node behind a material's base colour, through what's
    between them (MPFB mixes some with a colour)."""
    bsdf = next((n for n in tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return None
    todo = [link.from_node for link in tree.links if link.to_node == bsdf and link.to_socket.name == "Base Color"]
    seen = set()
    while todo:
        node = todo.pop(0)
        if node.name in seen:
            continue
        seen.add(node.name)
        if node.type == "TEX_IMAGE" and node.image:
            return node
        todo += [link.from_node for link in tree.links if link.to_node == node]
    return None


def recolour(obj, colour):
    """Repaints obj's base colour textures colour (sRGB, as textures hold
    it), keeping their light and dark (folds, seams, wear) and a little of
    their own colour: an item in a faction's colours."""
    for slot in obj.material_slots:
        tree = slot.material.node_tree if slot.material else None
        node = base_colour_node(tree) if tree else None
        if node is None:
            continue
        old = node.image
        w, h = old.size
        px = numpy.empty(w * h * 4, dtype=numpy.float32)
        old.pixels.foreach_get(px)
        px = px.reshape(-1, 4)
        lum = px[:, :3] @ numpy.array([0.299, 0.587, 0.114], dtype=numpy.float32)
        used = px[:, 3] > 0.5
        mid = float(numpy.median(lum[used])) if used.any() else float(lum.mean())
        shade = numpy.clip(lum / max(mid, 1e-3), 0.35, 1.6)[:, None]
        paint = numpy.clip(shade * numpy.array(colour, dtype=numpy.float32), 0.0, 1.0)
        px[:, :3] = (1 - RECOLOUR_KEEP) * paint + RECOLOUR_KEEP * px[:, :3]
        # A new image, so the item's plain version (and its file) keeps its own.
        new = bpy.data.images.new(old.name + " recoloured", w, h, alpha=True)
        new.colorspace_settings.name = old.colorspace_settings.name
        new.pixels.foreach_set(px.ravel())
        new.update()
        node.image = new


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
    bake(obj)
    tree = surface(inner, extra)
    world = obj.matrix_world
    normals = world.to_3x3().inverted().transposed()
    mesh = obj.data
    near = neighbours(mesh)
    # Work out every move before making any: moving a vertex makes Blender
    # recompute the normals from the half-moved mesh.
    cos = [world @ v.co for v in mesh.vertices]
    dirs = [(normals @ v.normal).normalized() for v in mesh.vertices]
    need = [0.0] * len(cos)
    for i, (co, n) in enumerate(zip(cos, dirs)):
        top = outermost(tree, co - n * LAYER_REACH, n, LAYER_REACH + gap)
        if top is not None and top + gap > LAYER_REACH:
            need[i] = top + gap - LAYER_REACH
    # Push along the smoothed normal.
    for _ in range(LAYER_SMOOTH):
        dirs = [(d + sum((dirs[j] for j in near[i]), Vector()) / max(len(near[i]), 1)).normalized()
                for i, d in enumerate(dirs)]
    moved = push_out(obj, near, cos, dirs, need, LAYER_SPREAD, LAYER_SMOOTH)
    print("LAYER", obj.name, "moved", moved, "of", len(obj.data.vertices))


def bake(obj):
    """Applies obj's shape keys and smoothing, so it's the final surface
    that moves. Leaves obj active and selected."""
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    if obj.data.shape_keys:
        bpy.ops.object.shape_key_remove(all=True, apply_mix=True)
    for mod in list(obj.modifiers):
        if mod.type == "SUBSURF":
            bpy.ops.object.modifier_apply(modifier=mod.name)


def neighbours(mesh):
    """Each vertex's neighbours along mesh's edges."""
    near = [[] for _ in mesh.vertices]
    for e in mesh.edges:
        a, b = e.vertices
        near[a].append(b)
        near[b].append(a)
    return near


def push_out(obj, near, cos, dirs, need, spread, smooth):
    """Moves each of obj's vertices (at cos, world space) along dirs by at
    least need: by the push widened over spread rings of edges, then
    smoothed over smooth, so cloth bulges smoothly over what's under it.
    Returns how many moved."""
    push = need
    for _ in range(spread):
        push = [max([p] + [push[j] for j in near[i]]) for i, p in enumerate(push)]
    for _ in range(smooth):
        push = [(p + sum(push[j] for j in near[i])) / (1 + len(near[i])) for i, p in enumerate(push)]
    back = obj.matrix_world.inverted()
    moved = 0
    for i, v in enumerate(obj.data.vertices):
        p = max(push[i], need[i])
        if p > 1e-5:
            v.co = back @ (cos[i] + dirs[i] * p)
            moved += 1
    obj.data.update()
    return moved


def drape(obj, body, regions, below):
    """Hangs obj's cloth over the body regions from where it rests, as it
    would fall. Round the body's axis, each way, it measures the body's
    outline seen from the side (how far out it is at each height) and takes
    its hull, which spans hollows, the waist and the small of the back, in
    straight lines from what holds the cloth out above and below, and round
    the body, each height's outline filled out to its convex hull (over the
    spine, between the breasts). Cloth hangs only lower than below: above,
    the shoulders hold it. Cloth over the regions is moved out to sit over
    the hull as far as it sat over the body; nothing moves in."""
    bake(obj)
    world = obj.matrix_world
    mesh = obj.data
    cos = [world @ v.co for v in mesh.vertices]
    verts, _ = body.skin()
    faces = [body.obj.data.polygons[f].vertices[:] for r in regions for f in body.region_faces.get(r, [])]
    trunk = sorted({v for f in faces for v in f})
    if not trunk:
        return 0
    surface_ = BVHTree.FromPolygons(verts, faces)
    axis = sum((verts[v] for v in trunk), Vector()) / len(trunk)
    bottom = min(verts[v].z for v in trunk)
    low = max(bottom, min(co.z for co in cos)) - DRAPE_STEP
    high = max(co.z for co in cos)
    if low >= below:
        return 0
    rows = int((high - low) / DRAPE_STEP) + 2
    heights = [low + k * DRAPE_STEP for k in range(rows)]
    # outline[b][k] and hull[b][k]: how far out the body and its hull are at
    # angle bin b and height heights[k] (None where the body isn't).
    ways = [Vector((math.cos(a), math.sin(a))) for a in (2 * math.pi * b / DRAPE_BINS for b in range(DRAPE_BINS))]
    outline = [[None] * rows for _ in ways]
    for b, out in enumerate(ways):
        for k, z in enumerate(heights):
            # From well outside, in towards the axis: the first hit is the
            # body's outermost surface that way.
            o = out.to_3d()
            hit = surface_.ray_cast(Vector((axis.x, axis.y, z)) + o, -o, 1.0)[0]
            if hit is not None:
                outline[b][k] = Vector((hit.x - axis.x, hit.y - axis.y)).length
    # Round the body first, each height's outline filled out to its convex
    # hull (over the spine, between the breasts), easing out up to the
    # shoulders (higher, it would span the neck from shoulder to shoulder),
    # then up it.
    shoulders = below + ARMPIT
    across = [row[:] for row in outline]
    for k, z in enumerate(heights):
        points = [(ways[b] * outline[b][k])[:] for b in range(DRAPE_BINS) if outline[b][k] is not None]
        t = min(1.0, max(0.0, (shoulders - z) / DRAPE_FADE))
        if len(points) < 3 or t == 0:
            continue
        ring = convex_hull(points)
        for b, out in enumerate(ways):
            if outline[b][k] is not None:
                fill = max(outline[b][k], ray_to_hull(ring, out))
                across[b][k] = outline[b][k] + (fill - outline[b][k]) * t * t * (3 - 2 * t)
    # Then up it, below the armpits (easing in), where it hangs.
    hull = []
    for b in range(DRAPE_BINS):
        h = upper_hull([(z, r) for z, r in zip(heights, across[b]) if r is not None and z <= below])
        column = []
        for z, r in zip(heights, across[b]):
            if r is None or not h:
                column.append(r)
                continue
            t = min(1.0, max(0.0, (below - z) / DRAPE_FADE))
            column.append(r + (max(r, hull_at(h, z)) - r) * t * t * (3 - 2 * t))
        hull.append(column)

    def at(grid, co):
        """grid at co, between the bins and heights round it."""
        d = Vector((co.x - axis.x, co.y - axis.y))
        fb = (math.atan2(d.y, d.x) % (2 * math.pi)) / (2 * math.pi) * DRAPE_BINS
        fk = (co.z - low) / DRAPE_STEP
        if fk < 0 or fk > rows - 1.001:
            return None
        b0, k0 = int(fb) % DRAPE_BINS, int(fk)
        total = weight = 0.0
        for b, wb in ((b0, 1 - (fb - int(fb))), ((b0 + 1) % DRAPE_BINS, fb - int(fb))):
            for k, wk in ((k0, 1 - (fk - k0)), (k0 + 1, fk - k0)):
                if grid[b][k] is not None and wb * wk > 0:
                    total += grid[b][k] * wb * wk
                    weight += wb * wk
        return total / weight if weight > 0 else None

    near = neighbours(mesh)
    # How far over the body each vertex sits, smoothed along the cloth: one
    # layer keeps its place over another (a bib over a shirt, in one item),
    # but not the shape of the hollows it's lifted out of.
    lift, dirs, hull_rs = [None] * len(cos), [Vector((0, 0, 0)) for _ in cos], [None] * len(cos)
    for i, co in enumerate(cos):
        d = Vector((co.x - axis.x, co.y - axis.y, 0))
        body_r, hull_r = at(outline, co), at(hull, co)
        if d.length < 1e-6 or body_r is None or hull_r is None or d.length > body_r + DRAPE_NEAR:
            continue  # off the trunk: a sleeve, say
        dirs[i], hull_rs[i] = d.normalized(), hull_r
        lift[i] = max(DRAPE_GAP, d.length - body_r)
    for _ in range(DRAPE_LIFT_SMOOTH):
        lift = [None if l is None else (l + sum(lift[j] for j in near[i] if lift[j] is not None))
                / (1 + sum(1 for j in near[i] if lift[j] is not None)) for i, l in enumerate(lift)]
    need = [0.0] * len(cos)
    for i, co in enumerate(cos):
        if lift[i] is not None:
            r = Vector((co.x - axis.x, co.y - axis.y)).length
            need[i] = min(DRAPE_MOST, max(0.0, hull_rs[i] + lift[i] - r))
    moved = push_out(obj, near, cos, dirs, need, 0, LAYER_SMOOTH)
    print("DRAPE", obj.name, "moved", moved, "of", len(cos))
    return moved


def convex_hull(points):
    """The convex hull of points (x, y), anticlockwise."""
    pts = sorted(set(points))
    if len(pts) < 3:
        return pts

    def half(ps):
        h = []
        for p in ps:
            while len(h) >= 2 and ((h[-1][0] - h[-2][0]) * (p[1] - h[-2][1])
                                   - (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0])) <= 0:
                h.pop()
            h.append(p)
        return h
    lower, upper = half(pts), half(reversed(pts))
    return lower[:-1] + upper[:-1]


def ray_to_hull(ring, direction):
    """How far from the origin (inside ring, a convex polygon) the ray
    along direction leaves it."""
    best = 0.0
    for (ax, ay), (bx, by) in zip(ring, ring[1:] + ring[:1]):
        ex, ey = bx - ax, by - ay
        den = direction.x * ey - direction.y * ex
        if abs(den) < 1e-12:
            continue
        t = (ax * ey - ay * ex) / den
        u = (ax * direction.y - ay * direction.x) / den
        if t > 0 and -1e-9 <= u <= 1 + 1e-9:
            best = max(best, t)
    return best


def upper_hull(points):
    """The upper hull of points (x, y), sorted by x."""
    hull = []
    for p in points:
        while len(hull) >= 2 and ((hull[-1][0] - hull[-2][0]) * (p[1] - hull[-2][1])
                                  - (hull[-1][1] - hull[-2][1]) * (p[0] - hull[-2][0])) >= 0:
            hull.pop()
        hull.append(p)
    return hull


def hull_at(hull, x):
    """The hull's height at x (within it), along its edges."""
    i = bisect.bisect_left(hull, (x, -math.inf))
    if i <= 0:
        return hull[0][1]
    if i >= len(hull):
        return hull[-1][1]
    (x0, y0), (x1, y1) = hull[i - 1], hull[i]
    return y0 if x1 <= x0 else y0 + (y1 - y0) * (x - x0) / (x1 - x0)


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
    # save_render puts the image through the scene's view transform (AgX by
    # default), which darkens and saturates a texture: keep it as it is.
    view = scene.view_settings
    view.view_transform, view.look, view.exposure, view.gamma = "Standard", "None", 0, 1
    scene.render.image_settings.file_format = "JPEG"
    scene.render.image_settings.quality = 85
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, name + ".jpg")
    image.save_render(path, scene=scene)
    bpy.data.images.remove(image)
    return path


def export_face(body, gender, skin, face, path):
    """Builds a person with face (a cast.FACES entry's targets over cast.FACE)
    and exports their head, eyes and eyebrows, the head as a skin patch.
    body is the person it's for, whose neck it has to meet."""
    for target in face:
        if target.split("/")[0] in ("head", "neck"):
            raise SystemExit("%s: a face can't change the head's or neck's shape (%s)" % (path, target))
    people.clear_scene()
    basemesh, rig = people.make_person(gender, skin, [], dict(FACE, **face))
    other = Body(basemesh)
    apart = max(((other.verts[v][0] - body.verts[v][0]).length
                 for v in other.region_verts["head"] if len(other.vert_regions[v]) > 1), default=0)
    if apart > SEAM:
        raise SystemExit("%s: the neck is %.1f mm off the body's" % (path, apart * 1000))
    people.split_body(basemesh)
    head = None
    for obj in list(bpy.data.objects):
        if obj.name == people.REGION_PREFIX + "head":
            head = obj
        elif obj.name.startswith(people.REGION_PREFIX):
            bpy.data.objects.remove(obj, do_unlink=True)
    head.name = SKIN_PREFIX + "head"
    head.data.materials.clear()
    head.data.materials.append(skin_material())
    features = [o for o in bpy.data.objects if o.type == "MESH" and o != head]
    # The eyebrows' texture is cut out by its alpha.
    export_item(head, features, rig, path, True)


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
    # Normalised, so a trailing slash doesn't throw off the paths made
    # relative to its parent.
    out = os.path.normpath(sys.argv[sys.argv.index("--") + 1])
    wardrobe = {"bodies": []}
    for name, (gender, skin, underclothes) in PEOPLE.items():
        body_glb = glb_json(os.path.join(out, name + ".glb"))
        body_joints = joint_names(body_glb)
        indices = mesh_indices(body_glb)
        regions, underwear, skin_meshes = {}, {}, []
        # The body's eyes and eyebrows: what a face replaces besides the head.
        face_meshes = [i for node, idx in indices.items() if not node.startswith(people.REGION_PREFIX) for i in idx]
        for node, idx in indices.items():
            if node.startswith(people.REGION_PREFIX):
                parts = node[len(people.REGION_PREFIX):].split(".")
                if len(parts) == 1:  # the body itself
                    regions.setdefault(parts[0], []).extend(idx)
                    skin_meshes.extend(idx)
                else:  # underwear on the region
                    underwear.setdefault(parts[0], []).extend(idx)

        people.clear_scene()
        skin_name = skin  # the items' loop below reuses the name
        basemesh, rig = people.make_person(gender, skin, underclothes)
        body = Body(basemesh)
        shoulder = rig.matrix_world @ rig.data.bones["upperarm_l"].head_local
        armpit = shoulder.z - ARMPIT
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
            for item, label, *colour in items:
                obj = fit(slot, item, basemesh)
                if colour:
                    recolour(obj, colour[0])
                hides, covers, patches = [], [], []
                if slot in DRAPED:
                    drape(obj, body, DRAPED[slot], armpit)
                if slot in WORN_OVER:
                    layer_over(obj, underwear_objs, LAYER_GAP, [body.skin()])
                    clothes = [o for s in WORN_OVER[slot] for o in kept.get(s, [])]
                    if clothes:
                        layer_over(obj, clothes, CLOTHES_GAP)
                if slot not in OVER_FACE:
                    over, share = body.covered(obj)
                    hides = sorted(r for r, x in share.items() if x >= HIDES)
                    under = underwear_covered(obj, underwear_objs)
                    covers = sorted(r for r, x in under.items() if x >= COVERED)
                    # A top replaces the torso undergarment as a whole, including
                    # straps and cups visible through a neckline or keyhole.
                    if slot in ("top", "outfit") and "torso" in underwear and "torso" not in covers:
                        covers.append("torso")
                        covers.sort()
                    patches = [p for p in (body.patch(r, over, set(hides)) for r in hides) if p]
                # A repainted item is named for what it's called: the
                # plain one may be in the catalogue too.
                stem = item if not colour else item + "_" + label.lower().replace(" ", "_")
                path = os.path.join(out, name, slot, stem + ".glb")
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

        # The faces last: each is a person of its own, built in a new scene.
        for item, label, face in FACES:
            path = os.path.join(out, name, "face", item + ".glb")
            export_face(body, gender, skin_name, face, path)
            glb = glb_json(path)
            if joint_names(glb) != body_joints:
                raise SystemExit("%s: %s's joints don't match the body's" % (name, item))
            skin_patches = [i for node, idx in mesh_indices(glb).items() if node.startswith(SKIN_PREFIX) for i in idx]
            slots.setdefault("face", []).append({
                "name": label, "path": os.path.relpath(path, os.path.dirname(out)),
                "hides": ["head"], "covers": [], "skinMeshes": skin_patches})
            print("FACE", name, item, "%.0f KB" % (os.path.getsize(path) / 1024))

        # The factions' looks: each slot's item by name, or None to take off
        # what's there.
        looks = []
        for look, wear in LOOKS.get(name, {}).items():
            for slot, label in wear.items():
                if label is not None and label not in [it["name"] for it in slots.get(slot, [])]:
                    raise SystemExit("%s: the %s look wears %r in %s, which isn't in the catalogue" % (name, look, label, slot))
            looks.append({"name": look, "wear": wear})

        wardrobe["bodies"].append({
            "name": name,
            "model": os.path.relpath(os.path.join(out, name + ".glb"), os.path.dirname(out)),
            "skinMeshes": skin_meshes,
            "regions": regions,
            "faceMeshes": face_meshes,
            "underwear": underwear,
            "skins": skins,
            "slots": slots,
            "looks": looks,
        })
    with open(os.path.join(out, "wardrobe.json"), "w") as f:
        json.dump(wardrobe, f, indent=1)
    print("WARDROBE", os.path.join(out, "wardrobe.json"))


if __name__ == "__main__":
    main()
