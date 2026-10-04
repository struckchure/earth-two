"""Builds Earth Two's people with MPFB (MakeHuman for Blender).

Run it in Blender with MPFB installed and the MakeHuman system assets
loaded (see the README):

    blender -b --python tools/makehuman/people.py -- UAL.glb OUT_DIR MIXAMO_DIR

Each person is a bare-skinned MakeHuman base mesh with eyes, eyebrows and
underwear, rigged with MPFB's game engine rig, animated with clips from
Quaternius's Universal Animation Library (UAL.glb, the in-place version) and
Mixamo (MIXAMO_DIR, see MIXAMO_CLIPS), split into body regions (see
REGIONS), and exported to OUT_DIR as a .glb.

The library's skeleton has the same Unreal-mannequin bone names as the game
engine rig, and Mixamo's maps onto it by MIXAMO_BONES, so clips transfer
bone by bone. The rest poses differ (theirs are T-poses, MakeHuman's an
A-pose), so each bone's rotation is carried over relative to a reference
pose: the MakeHuman rig posed to match the source's rest pose.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from cast import CLIPS, FACE, MIXAMO_CLIPS, PEOPLE
from traversal import author as author_traversal

import bpy
from mathutils import Matrix, Quaternion, Vector

from bl_ext.user_default.mpfb.services import HumanService, LocationService, TargetService

DATA = LocationService.get_user_data()


def mixamo_bones():
    """Mixamo's bone names mapped to the game engine rig's."""
    names = {"Hips": "pelvis", "Spine": "spine_01", "Spine1": "spine_02", "Spine2": "spine_03",
             "Neck": "neck_01", "Head": "head"}
    for side, s in (("Left", "l"), ("Right", "r")):
        names.update({side + "Shoulder": "clavicle_" + s, side + "Arm": "upperarm_" + s,
                      side + "ForeArm": "lowerarm_" + s, side + "Hand": "hand_" + s,
                      side + "UpLeg": "thigh_" + s, side + "Leg": "calf_" + s,
                      side + "Foot": "foot_" + s, side + "ToeBase": "ball_" + s})
        for finger in ("Thumb", "Index", "Middle", "Ring", "Pinky"):
            for i in (1, 2, 3):
                names["%sHand%s%d" % (side, finger, i)] = "%s_%02d_%s" % (finger.lower(), i, s)
    return {"mixamorig:" + k: v for k, v in names.items()}


MIXAMO_BONES = mixamo_bones()

# Every clip is sampled at this rate: Mixamo's. The FBX importer sets the
# scene to it anyway, and the glTF importer keys the library's clips by it.
FPS = 30

# The region each underwear piece sits on (see split_body): it's hidden with
# the region, so it can't poke through the clothes worn over it.
UNDERWEAR_REGION = {
    "wojackowl_boxer_shorts": "hips",
    "wolgade_female_panties_01": "hips",
    "wolgade_female_top_01": "torso",
}

# Body regions. The body is split into one mesh per region, each vertex going
# with the bone that moves it most, so the game can hide what clothes cover
# (tools/makehuman/wardrobe.py works out which regions each garment covers).
REGIONS = {
    "head": ["head", "neck_01"],
    "torso": ["spine_01", "spine_02", "spine_03", "clavicle_l", "clavicle_r"],
    "hips": ["Root", "pelvis"],
    "upperarms": ["upperarm_l", "upperarm_r"],
    "forearms": ["lowerarm_l", "lowerarm_r"],
    "hands": ["hand_l", "hand_r"],  # and the fingers, see bone_region
    "thighs": ["thigh_l", "thigh_r"],
    "calves": ["calf_l", "calf_r"],
    "feet": ["foot_l", "foot_r", "ball_l", "ball_r"],
}
# Objects in a region are named REGION_PREFIX + region (+ "." + piece).
REGION_PREFIX = "region."

# Clothes textures are shrunk to this many pixels a side: MakeHuman's are up
# to 4096, far more than a game body needs, and the web build downloads them.
CLOTHES_TEXTURE_SIZE = 1024


def clear_scene():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for block in (bpy.data.meshes, bpy.data.armatures, bpy.data.materials, bpy.data.actions):
        for item in list(block):
            if item.users == 0:
                block.remove(item)


def asset(kind, name, ext):
    return os.path.join(DATA, kind, name, name + ext)


def drop_see_through_faces(obj):
    """Deletes the faces of obj that sit on the transparent part of its
    texture: the eyes' corneas. raylib draws them invisible but still hides
    what's behind them, the eyeballs."""
    import bmesh

    mat = obj.active_material
    image = next(n.image for n in mat.node_tree.nodes if n.type == "TEX_IMAGE" and n.image)
    w, h = image.size
    pixels = image.pixels[:]
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    uv = bm.loops.layers.uv.active
    see_through = []
    for face in bm.faces:
        u = sum(loop[uv].uv.x for loop in face.loops) / len(face.loops)
        v = sum(loop[uv].uv.y for loop in face.loops) / len(face.loops)
        x = min(w - 1, max(0, int(u % 1 * w)))
        y = min(h - 1, max(0, int(v % 1 * h)))
        if pixels[(y * w + x) * 4 + 3] < 0.5:
            see_through.append(face)
    bmesh.ops.delete(bm, geom=see_through, context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
    mat.surface_render_method = "DITHERED"  # exports as opaque
    print("EYES", len(see_through), "see-through faces dropped")


def shrink_textures(obj, size):
    """Scales obj's material textures down to at most size pixels a side."""
    for slot in obj.material_slots:
        if not slot.material or not slot.material.node_tree:
            continue
        for node in slot.material.node_tree.nodes:
            image = node.image if node.type == "TEX_IMAGE" else None
            if image and max(image.size) > size:
                w, h = image.size
                k = size / max(w, h)
                image.scale(max(1, int(w * k)), max(1, int(h * k)))


def shape_face(basemesh, face):
    """Loads the targets in face (see FACE) onto basemesh."""
    targets = os.path.join(LocationService.get_mpfb_data("targets"))
    for name, weight in face.items():
        for side in ("l-", "r-") if "?-" in name else ("",):
            path = os.path.join(targets, name.replace("?-", side) + ".target.gz")
            TargetService.load_target(basemesh, path, weight=weight)


def make_person(gender, skin, clothes, face=FACE):
    macro = TargetService.get_default_macro_info_dict()
    macro["gender"] = gender
    macro["race"] = {"african": 1.0, "asian": 0.0, "caucasian": 0.0}
    # detailed_helpers keeps the joint-* vertex groups the rig is fitted to.
    basemesh = HumanService.create_human(macro_detail_dict=macro)
    # Before the rig and the eyes, which are fitted to the face.
    shape_face(basemesh, face)
    HumanService.set_character_skin(asset("skins", skin, ".mhmat"), basemesh, skin_type="GAMEENGINE", material_instances=False)
    # The rig first, so the eyes and eyebrows are bound to it too.
    rig = HumanService.add_builtin_rig(basemesh, "game_engine")
    eyes = HumanService.add_mhclo_asset(asset("eyes", "high-poly", ".mhclo"), basemesh, asset_type="Eyes", material_type="GAMEENGINE")
    drop_see_through_faces(eyes)
    HumanService.add_mhclo_asset(asset("eyebrows", "eyebrow001", ".mhclo"), basemesh, asset_type="Eyebrows", material_type="GAMEENGINE")
    for item in clothes:
        worn = HumanService.add_mhclo_asset(asset("clothes", item, ".mhclo"), basemesh, asset_type="Clothes", material_type="GAMEENGINE")
        shrink_textures(worn, CLOTHES_TEXTURE_SIZE)
        if item in UNDERWEAR_REGION:
            worn.name = REGION_PREFIX + UNDERWEAR_REGION[item] + "." + item
    return basemesh, rig


def bone_region(bone):
    for region, bones in REGIONS.items():
        if bone in bones:
            return region
    if bone.split("_")[0] in ("thumb", "index", "middle", "ring", "pinky"):
        return "hands"
    return None


def vertex_regions(obj):
    """Each vertex's region: that of the bone weighing on it most."""
    group_region = {g.index: bone_region(g.name) for g in obj.vertex_groups}
    regions = []
    for v in obj.data.vertices:
        best, region = 0.0, "torso"
        for g in v.groups:
            r = group_region.get(g.group)
            if r and g.weight > best:
                best, region = g.weight, r
        regions.append(region)
    return regions


def face_regions(obj):
    """Each face's region: the one most of its vertices are in."""
    regions = vertex_regions(obj)
    out = []
    for poly in obj.data.polygons:
        votes = {}
        for v in poly.vertices:
            votes[regions[v]] = votes.get(regions[v], 0) + 1
        out.append(max(votes, key=votes.get))
    return out


def prepare_body(basemesh):
    """Bakes the body's shape (MPFB keeps its targets as shape keys) so the
    mesh can be cut, and freezes its normals so cuts don't show as seams.
    Leaves basemesh active and selected."""
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = basemesh
    basemesh.select_set(True)
    if basemesh.data.shape_keys:
        bpy.ops.object.shape_key_remove(all=True, apply_mix=True)
    mesh = basemesh.data
    mesh.normals_split_custom_set_from_vertices([v.normal[:] for v in mesh.vertices])


def split_body(basemesh):
    """Splits the body into one object per region, named REGION_PREFIX +
    region, so each exports as its own mesh."""
    prepare_body(basemesh)
    mesh = basemesh.data
    first, *rest = REGIONS
    for region in rest:
        faces = face_regions(basemesh)
        for poly, r in zip(mesh.polygons, faces):
            poly.select = r == region
        before = set(bpy.data.objects)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.separate(type="SELECTED")
        bpy.ops.object.mode_set(mode="OBJECT")
        for obj in set(bpy.data.objects) - before:
            obj.name = REGION_PREFIX + region
            obj.select_set(False)
    basemesh.name = REGION_PREFIX + first


def load_library(path):
    """Imports the animation library; returns its armature."""
    bpy.ops.import_scene.gltf(filepath=path)
    source = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    for obj in list(bpy.context.scene.objects):
        if obj != source:
            bpy.data.objects.remove(obj, do_unlink=True)
    return source


def load_mixamo(folder, clips, target):
    """Imports the Mixamo clips onto one armature, turned to face the way
    target does; returns it."""
    source = None
    for clip in clips:
        before = set(bpy.data.objects)
        bpy.ops.import_scene.fbx(filepath=os.path.join(folder, clip + ".fbx"))
        new = [o for o in bpy.data.objects if o not in before]
        arm = next(o for o in new if o.type == "ARMATURE")
        action = arm.animation_data.action
        action.name = clip
        action.use_fake_user = True
        if source is None:
            source = arm
            new.remove(arm)
        for obj in new:
            bpy.data.objects.remove(obj, do_unlink=True)
    source.animation_data.action = None
    bpy.context.view_layer.update()
    want, have = facing(target, "foot_l", "ball_l"), facing(source, "mixamorig:LeftFoot", "mixamorig:LeftToeBase")
    turn = math.atan2(want.y, want.x) - math.atan2(have.y, have.x)
    source.matrix_world = Matrix.Rotation(turn, 4, "Z") @ source.matrix_world
    bpy.context.view_layer.update()
    return source


def facing(arm, foot, toe):
    """The way arm's skeleton faces at rest: along its foot, on the ground."""
    w = arm.matrix_world
    v = w @ arm.data.bones[toe].head_local - w @ arm.data.bones[foot].head_local
    return Vector((v.x, v.y, 0)).normalized()


def bone_map(source, target, names=None):
    """Pairs target bones with source bones: by names (source to target), or
    else those of the same name, ignoring case."""
    if names:
        return {t: s for s, t in names.items() if s in source.pose.bones and t in target.pose.bones}
    names = {b.name.lower(): b.name for b in source.pose.bones}
    return {t.name: names[t.name.lower()] for t in target.pose.bones if t.name.lower() in names}


def world_rotation(obj, pose_bone):
    return (obj.matrix_world @ pose_bone.matrix).to_quaternion().normalized()


def world_rest_rotation(obj, bone):
    return (obj.matrix_world @ bone.matrix_local).to_quaternion().normalized()


def parents_first(armature):
    order = []

    def visit(bone):
        order.append(bone.name)
        for child in bone.children:
            visit(child)

    for bone in armature.data.bones:
        if bone.parent is None:
            visit(bone)
    return order


# The child joint each bone is aimed at in the reference pose, where a bone
# has several; a bone with one mapped child aims at it, and one with none
# along its own length.
MAIN_CHILD = {"hand_l": "middle_01_l", "hand_r": "middle_01_r"}
# Bones that keep their own rest pose in the reference pose. Both skeletons
# stand upright at rest, and each spine sits in its own body differently, so
# lining MakeHuman's spine up joint for joint with the library's would lean
# the body. The clavicles too: the library's T-pose holds its shoulders
# back, and aiming MakeHuman's clavicles the same way pulls its shoulders
# 10cm back and up, which pushes the chest out.
UPRIGHT = {"Root", "pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "head", "clavicle_l", "clavicle_r"}
# Bones whose sideways axis follows two of their children (the palm's width).
ACROSS = {"hand_l": ("index_01_l", "pinky_01_l"), "hand_r": ("index_01_r", "pinky_01_r")}


def frame(primary, secondary):
    """A rotation whose Y axis is primary and X axis lies toward secondary."""
    y = primary.normalized()
    x = (secondary - y * secondary.dot(y)).normalized()
    z = x.cross(y)
    return Matrix((x, y, z)).transposed().to_quaternion()


def aim(armature, bone_name, pairs, head_of):
    """The direction a bone points at: its main child's joint, or its tail."""
    bone = armature.data.bones[bone_name]
    kids = [c.name for c in bone.children if c.name in pairs]
    child = MAIN_CHILD.get(bone_name) or (kids[0] if len(kids) == 1 else None)
    if child is None:
        return None
    return head_of(child) - head_of(bone_name)


def reference_pose(source, target, pairs):
    """Poses target so each limb bone lines up with its source bone at rest,
    aimed at the same child joint and, for the hands, turned the same way;
    returns each bone's world rotation in that pose."""
    for pb in target.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = Quaternion()
        pb.location = (0, 0, 0)
    bpy.context.view_layer.update()
    sw = source.matrix_world
    tw = target.matrix_world
    inverse = {t: s for t, s in pairs.items()}

    def src_head(name):
        return sw @ source.data.bones[inverse[name]].head_local

    def dst_head(name):
        return tw @ target.pose.bones[name].head

    for name in parents_first(target):
        if name not in pairs or name in UPRIGHT:
            continue
        pb = target.pose.bones[name]
        sb = source.data.bones[pairs[name]]
        want = aim(target, name, pairs, src_head)
        have = aim(target, name, pairs, dst_head)
        if want is None:
            want = sw.to_3x3() @ (sb.tail_local - sb.head_local)
            have = tw.to_3x3() @ (pb.tail - pb.head)
        world = tw @ pb.matrix
        if name in ACROSS:
            a, b = ACROSS[name]
            turn = frame(want, src_head(b) - src_head(a)) @ frame(have, dst_head(b) - dst_head(a)).inverted()
        else:
            turn = have.rotation_difference(want)
        loc = world.to_translation()
        rotated = Matrix.Translation(loc) @ turn.to_matrix().to_4x4() @ Matrix.Translation(-loc) @ world
        pb.matrix = tw.inverted() @ rotated
        bpy.context.view_layer.update()
    ref = {name: world_rotation(target, target.pose.bones[name]) for name in pairs}
    for pb in target.pose.bones:
        pb.rotation_quaternion = Quaternion()
    bpy.context.view_layer.update()
    return ref


BAKED = "~baked"  # suffix for baked clips while the library's are still loaded

# The relaxed idle: Idle_Loop's breathing in the spine, neck and head, and
# the rest of the body standing at ease. The library's idle is a fighter's
# ready stance (feet wide, knees bent, elbows out, fists), which looks stiff
# on everyday people; it stays in the file for fights.
RELAXED_IDLE = "Idle_Relaxed"
AT_EASE = {"Root", "pelvis", "clavicle_l", "clavicle_r", "thigh_l", "thigh_r", "calf_l", "calf_r",
           "foot_l", "foot_r", "ball_l", "ball_r"}
# Where each arm bone points when relaxed, for the left side (+X); the right
# mirrors it. Blender's front is -Y.
HANGING = {"upperarm_l": (0.16, 0.02, -1.0), "lowerarm_l": (0.06, -0.3, -1.0), "hand_l": (0.03, -0.2, -1.0)}


def point_bone(target, name, direction):
    """Turns a posed bone so it points along direction, in world space."""
    pb = target.pose.bones[name]
    tw = target.matrix_world
    have = (tw @ pb.tail - tw @ pb.head).normalized()
    turn = have.rotation_difference(Vector(direction).normalized())
    world = tw @ pb.matrix
    loc = world.to_translation()
    pb.matrix = tw.inverted() @ Matrix.Translation(loc) @ turn.to_matrix().to_4x4() @ Matrix.Translation(-loc) @ world
    bpy.context.view_layer.update()


def relaxed_idle(target):
    """Makes RELAXED_IDLE from the baked Idle_Loop (see AT_EASE, HANGING)."""
    for pb in target.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = Quaternion()
        pb.location = (0, 0, 0)
    bpy.context.view_layer.update()
    for name, (x, y, z) in HANGING.items():
        point_bone(target, name, (x, y, z))
        point_bone(target, name.replace("_l", "_r"), (-x, y, z))
    held = {pb.name: pb.rotation_quaternion.copy() for pb in target.pose.bones
            if pb.name in AT_EASE or pb.name.startswith(("upperarm", "lowerarm", "hand_", "thumb", "index", "middle", "ring", "pinky"))}

    idle = bpy.data.actions["Idle_Loop" + BAKED]
    action = idle.copy()
    action.name = RELAXED_IDLE + BAKED
    action.use_fake_user = True
    for fc in list(action.fcurves):
        bone = fc.data_path.split('"')[1] if '"' in fc.data_path else None
        if bone in held:
            action.fcurves.remove(fc)
    start, end = (int(round(f)) for f in idle.frame_range)
    target.animation_data_create()
    target.animation_data.action = action
    for name, q in held.items():
        pb = target.pose.bones[name]
        pb.rotation_quaternion = q
        pb.location = (0, 0, 0)
        for f in (start, end):
            pb.keyframe_insert("rotation_quaternion", frame=f)
            if name == "pelvis":
                pb.keyframe_insert("location", frame=f)
    # The ready stance leans the spine forward over its bent knees; keep only
    # its movement around the average, so the body stands straight and still
    # breathes.
    for name in ("spine_01", "spine_02", "spine_03", "neck_01", "head"):
        path = 'pose.bones["%s"].rotation_quaternion' % name
        curves = sorted((fc for fc in action.fcurves if fc.data_path == path), key=lambda fc: fc.array_index)
        if len(curves) != 4:
            continue
        keys = [Quaternion([c.keyframe_points[i].co[1] for c in curves]) for i in range(len(curves[0].keyframe_points))]
        mean = Quaternion((0, 0, 0, 0))
        for q in keys:
            mean += q if q.dot(keys[0]) >= 0 else -q
        mean.normalize()
        for i, q in enumerate(keys):
            steady = mean.inverted() @ q
            for c in curves:
                c.keyframe_points[i].co[1] = steady[c.array_index]
        for c in curves:
            c.update()
    target.animation_data.action = None
    for pb in target.pose.bones:
        pb.rotation_quaternion = Quaternion()
        pb.location = (0, 0, 0)


# The spine bones the torso correction turns, and the joints that bound the
# torso.
SPINE = ("spine_01", "spine_02", "spine_03")
TORSO = ("spine_01", "neck_01")


def torso_direction(arm, pairs, rest=False):
    """World direction from the bottom of arm's spine to its neck, posed or at
    rest; pairs maps target names to arm's, or None for the target itself."""
    names = [pairs[n] if pairs else n for n in TORSO]
    w = arm.matrix_world
    if rest:
        a, b = (w @ arm.data.bones[n].head_local for n in names)
    else:
        a, b = (w @ arm.pose.bones[n].head for n in names)
    return (b - a).normalized()


def torso_direction_of(target, pose):
    """torso_direction for target bones' armature-space matrices."""
    w = target.matrix_world
    a, b = (w @ pose[n].to_translation() for n in TORSO)
    return (b - a).normalized()


# Locomotion clips whose upper body is levelled, and how far (degrees) it
# then leans forward as one piece. The library leans a moving body by tilting
# the pelvis and bending the spine back against it; the mannequin barely
# shows it, but on MakeHuman, whose pelvis carries the belly and buttocks,
# it arches the lower back. So these clips keep only the pelvis, spine, neck
# and head's movement around their average, which is MakeHuman's upright
# rest, and lean it all together, from the hips, by this much.
LEVELLED = {"Walk_Loop": 0, "Walk_Formal_Loop": 0, "Jog_Fwd_Loop": 6, "Sprint_Loop": 12}
UPPER_BODY = ("pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "head")


def retarget(source, target, pairs, clip, ref, in_place=False):
    """Bakes source's clip onto target as a new action, clip + BAKED.
    in_place takes out the hips' net travel over the ground, for clips that
    walk off (the character controller does the moving)."""
    action = bpy.data.actions[clip]
    source.animation_data_create()
    source.animation_data.action = action
    if action.slots:
        source.animation_data.action_slot = action.slots[0]
    start, end = (int(round(f)) for f in action.frame_range)

    hips = "pelvis"
    src_rest_hips = source.matrix_world @ source.data.bones[pairs[hips]].head_local
    dst_rest_hips = target.matrix_world @ target.data.bones[hips].head_local
    scale = dst_rest_hips.z / src_rest_hips.z
    src_rest = {n: world_rest_rotation(source, source.data.bones[s]) for n, s in pairs.items()}
    order = [n for n in parents_first(target) if n in pairs]
    tw_inv = target.matrix_world.inverted()

    def turn(name):
        """How far source's bone is turned from its rest, in world space."""
        return world_rotation(source, source.pose.bones[pairs[name]]) @ src_rest[name].inverted()

    travel = Vector()
    if in_place:
        at = []
        for f in (start, end):
            bpy.context.scene.frame_set(f)
            at.append(source.matrix_world @ source.pose.bones[pairs[hips]].head)
        travel = at[1] - at[0]
        travel.z = 0
    slide = Vector()  # this frame's share of travel

    level = clip in LEVELLED
    mean = {}
    if level:
        sums = {n: Quaternion((0, 0, 0, 0)) for n in UPPER_BODY}
        for f in range(start, end + 1):
            bpy.context.scene.frame_set(f)
            for n in UPPER_BODY:
                q = turn(n)
                sums[n] += q if q.dot(Quaternion()) >= 0 else -q
        mean = {n: q.normalized() for n, q in sums.items()}
    lean = Quaternion((1, 0, 0), math.radians(LEVELLED.get(clip, 0)))  # about +X tips forward (-Y)

    def solve(tilt):
        """Target bones' armature-space matrices this frame, with the spine
        bones turned by tilt (a world-space rotation)."""
        pose = {}
        for name in order:
            bone = target.data.bones[name]
            delta = turn(name)
            if name in mean:
                delta = lean @ mean[name].inverted() @ delta
            want_rot = delta @ ref[name]
            if name in SPINE:
                want_rot = tilt @ want_rot
            # Where the bone would be with no local offset.
            if bone.parent is not None:
                parent = pose[bone.parent.name][1] if bone.parent.name in pose else bone.parent.matrix_local
                base = parent @ bone.parent.matrix_local.inverted() @ bone.matrix_local
            else:
                base = bone.matrix_local.copy()
            loc = base.to_translation()
            if name == hips:
                moved = (source.matrix_world @ source.pose.bones[pairs[hips]].head) - src_rest_hips - slide
                loc = tw_inv @ (dst_rest_hips + moved * scale)
            rot = (tw_inv.to_quaternion() @ want_rot).normalized()
            pose[name] = (base, Matrix.Translation(loc) @ rot.to_matrix().to_4x4())
        return pose

    # The torso: from the bottom of the spine to the neck, in world space.
    src_rest_torso = torso_direction(source, pairs, rest=True)
    dst_rest_torso = torso_direction(target, None, rest=True)

    frames = {}  # bone -> list of (rotation, location)
    for f in range(start, end + 1):
        bpy.context.scene.frame_set(f)
        slide = travel * ((f - start) / max(1, end - start))
        # The same spine rotations lean MakeHuman's torso further than the
        # library's (its spine segments are proportioned differently), so
        # turn the spine to lean the torso as much as the source's does.
        pose = solve(Quaternion())
        if not level:
            want = src_rest_torso.rotation_difference(torso_direction(source, pairs)) @ dst_rest_torso
            have = torso_direction_of(target, {n: m for n, (_, m) in pose.items()})
            tilt = have.rotation_difference(want)
            if abs(tilt.angle) > 1e-4:
                pose = solve(tilt)
        for name, (base, m) in pose.items():
            basis = base.inverted() @ m
            frames.setdefault(name, []).append((basis.to_quaternion(), basis.to_translation()))

    baked = bpy.data.actions.new(clip + BAKED)
    baked.use_fake_user = True
    target.animation_data_create()
    target.animation_data.action = baked
    for name, keys in frames.items():
        pb = target.pose.bones[name]
        pb.rotation_mode = "QUATERNION"
        for i, (q, t) in enumerate(keys):
            pb.rotation_quaternion = q
            pb.keyframe_insert("rotation_quaternion", frame=start + i)
            if name == hips:
                pb.location = t
                pb.keyframe_insert("location", frame=start + i)
    target.animation_data.action = None


def export(path):
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,  # the helper mask, not the armature
        export_morph=False,
        export_animations=True,
        export_animation_mode="ACTIONS",
    )


def main():
    library, out, mixamo = sys.argv[sys.argv.index("--") + 1:][:3]
    os.makedirs(out, exist_ok=True)
    for name, (gender, skin, clothes) in PEOPLE.items():
        clear_scene()
        for action in list(bpy.data.actions):
            bpy.data.actions.remove(action)
        bpy.context.scene.render.fps, bpy.context.scene.render.fps_base = FPS, 1
        source = load_library(library)
        basemesh, rig = make_person(gender, skin, clothes)
        pairs = bone_map(source, rig)
        ref = reference_pose(source, rig, pairs)
        for clip in CLIPS:
            retarget(source, rig, pairs, clip, ref)
        relaxed_idle(rig)
        mix = load_mixamo(mixamo, MIXAMO_CLIPS, rig)
        mix_pairs = bone_map(mix, rig, MIXAMO_BONES)
        mix_ref = reference_pose(mix, rig, mix_pairs)
        for clip in MIXAMO_CLIPS:
            retarget(mix, rig, mix_pairs, clip, mix_ref, in_place=True)
        bpy.data.objects.remove(mix, do_unlink=True)
        # Only the baked clips go in the file, under the clips' names.
        bpy.data.objects.remove(source, do_unlink=True)
        for action in list(bpy.data.actions):
            if not action.name.endswith(BAKED):
                bpy.data.actions.remove(action)
        for action in bpy.data.actions:
            action.name = action.name.removesuffix(BAKED)
        print("PERSON", name, len(pairs), "library and", len(mix_pairs), "Mixamo bones matched,", len(bpy.data.actions), "clips")
        author_traversal(rig, mixamo=mixamo)
        split_body(basemesh)
        export(os.path.join(out, name + ".glb"))


if __name__ == "__main__":
    main()
