# Prepares one of our characters for Mixamo's auto-rigger: its mesh alone,
# in the T-pose it's bound in, as one object 1.75 m tall with its feet at the
# origin, facing front.
#
#   blender -b --python tools/mixamo/upload.py -- character.glb out.fbx
import bpy, sys
src, out = sys.argv[sys.argv.index("--") + 1:]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
for o in meshes:
    for m in list(o.modifiers):
        o.modifiers.remove(m)
    o.animation_data_clear()
    world = o.matrix_world.copy()
    o.parent = None
    o.matrix_world = world
for o in [o for o in bpy.data.objects if o.type != "MESH"]:
    bpy.data.objects.remove(o)
for a in list(bpy.data.actions):
    bpy.data.actions.remove(a)
# One object: Mixamo rigs a single mesh.
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
if len(meshes) > 1:
    bpy.ops.object.join()
body = bpy.context.view_layer.objects.active
body.name = "Character"
for g in list(body.vertex_groups):
    body.vertex_groups.remove(g)
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
# Stand him 1.75 m tall with his feet on the ground at the origin.
body.scale *= 1.75 / body.dimensions.z
bpy.ops.object.transform_apply(scale=True)
lo = min((body.matrix_world @ v.co).z for v in body.data.vertices)
body.location.z -= lo
bpy.ops.object.transform_apply(location=True)
dims = body.dimensions
print("MESH", body.name, "verts", len(body.data.vertices), "materials", [m.name for m in body.data.materials], "size", tuple(round(d, 3) for d in dims))
bpy.ops.export_scene.fbx(filepath=out, use_selection=True, object_types={"MESH"}, add_leaf_bones=False, bake_anim=False, path_mode="COPY", embed_textures=True)
