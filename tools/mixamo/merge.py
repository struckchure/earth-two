# Merges Mixamo animations onto one of our characters, for raylib.
#
# Mixamo rigs a mesh we upload (tools/mixamo's README step: an unrigged,
# T-posed .fbx), and each animation downloads as its own "Without Skin" .fbx:
# a bare skeleton and one action. This binds the mesh to that skeleton with
# automatic weights, gathers every file's action onto it as a clip named after
# the file ("Walking.fbx" -> "Walking"), and writes one .glb.
#
# Each clip loses its hips' net horizontal travel, so clips downloaded without
# "In Place" still play in place (the character controller does the moving);
# the hips keep their sway and their height.
#
#   blender -b --python tools/mixamo/merge.py -- mesh.fbx out.glb anim.fbx...
import bmesh
import bpy
import os
import sys

args = sys.argv[sys.argv.index("--") + 1:]
mesh_path, out_path, anim_paths = args[0], args[1], args[2:]
if not anim_paths:
    sys.exit("usage: merge.py -- mesh.fbx out.glb anim.fbx...")

bpy.ops.wm.read_factory_settings(use_empty=True)


def import_fbx(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path)
    return [o for o in bpy.data.objects if o not in before]


def clip_name(path):
    return os.path.splitext(os.path.basename(path))[0]


def in_place(action):
    """Removes the hips' net travel on the ground plane (bone-local X and Z;
    Y runs up the hips bone)."""
    for fc in action.fcurves:
        if fc.data_path != 'pose.bones["mixamorig:Hips"].location' or fc.array_index not in (0, 2):
            continue
        keys = fc.keyframe_points
        if len(keys) < 2:
            continue
        (f0, v0), (f1, v1) = keys[0].co, keys[-1].co
        for k in keys:
            t = (k.co[0] - f0) / (f1 - f0) if f1 != f0 else 0
            shift = (v1 - v0) * t
            k.co[1] -= shift
            k.handle_left[1] -= shift
            k.handle_right[1] -= shift
        fc.update()


# The first animation's skeleton becomes the character's; the others only
# lend their actions.
arm = next(o for o in import_fbx(anim_paths[0]) if o.type == "ARMATURE")
arm.name = "Armature"
# The FBX importer sets the scene's frame rate from each file; keep the
# animations' (Mixamo's 30 fps), which the export samples at.
fps, fps_base = bpy.context.scene.render.fps, bpy.context.scene.render.fps_base
actions = []
for i, path in enumerate(anim_paths):
    if i == 0:
        act = arm.animation_data.action
    else:
        objs = import_fbx(path)
        other = next(o for o in objs if o.type == "ARMATURE")
        act = other.animation_data.action
        for o in objs:
            bpy.data.objects.remove(o)
    act.name = clip_name(path)
    in_place(act)
    actions.append(act)

# Every clip on its own NLA track, which the glTF exporter turns into one
# animation each, named after the track.
ad = arm.animation_data
ad.action = None
for act in actions:
    track = ad.nla_tracks.new()
    track.name = act.name
    start = int(act.frame_range[0])
    strip = track.strips.new(act.name, start, act)
    track.mute = True

# Bind the mesh, in the skeleton's rest pose.
bpy.context.scene.frame_set(0)
mesh = next(o for o in import_fbx(mesh_path) if o.type == "MESH")
for o in list(bpy.data.objects):
    if o not in (arm, mesh):
        bpy.data.objects.remove(o)

# Flat-shaded low-poly meshes arrive with every triangle's corners separate,
# which automatic weighting can't solve. Weld them, and keep each face flat so
# the export still looks faceted.
bm = bmesh.new()
bm.from_mesh(mesh.data)
bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
for f in bm.faces:
    f.smooth = False
bm.to_mesh(mesh.data)
bm.free()
bpy.ops.object.select_all(action="DESELECT")
mesh.select_set(True)
arm.select_set(True)
bpy.context.view_layer.objects.active = arm
bpy.ops.object.parent_set(type="ARMATURE_AUTO")

# Every vertex needs a bone, or it stays behind when the body moves.
unweighted = sum(1 for v in mesh.data.vertices if not any(g.weight > 0 for g in v.groups))
print("MERGE mesh %s: %d vertices, %d unweighted; skeleton: %d bones; clips: %s" % (
    mesh.name, len(mesh.data.vertices), unweighted, len(arm.data.bones), [a.name for a in actions]))
if unweighted:
    sys.exit("automatic weights left %d vertices without a bone" % unweighted)

bpy.context.scene.render.fps, bpy.context.scene.render.fps_base = fps, fps_base
bpy.ops.export_scene.gltf(
    filepath=out_path,
    export_format="GLB",
    export_animation_mode="NLA_TRACKS",
    export_force_sampling=True,
    export_def_bones=True,
    export_leaf_bone=False,
    export_morph=False,
)
