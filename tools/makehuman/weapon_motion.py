"""Retarget the supplied Mixamo-rigged strafe and mirror it for both directions.
blender -b --python tools/makehuman/weapon_motion.py -- assets/characters build/weapon-motion
Then: python3 tools/makehuman/merge_weapon_motion.py assets/characters build/weapon-motion
Run after traversal generation; preserves original meshes through the merge step.
"""
import os, sys
import bpy
from mathutils import Quaternion, Vector
sys.path.insert(0,os.path.dirname(__file__))
from traversal import capture, orient, mirror_clip

folder,out=map(os.path.abspath,sys.argv[sys.argv.index('--')+1:])
os.makedirs(out,exist_ok=True)
reference=os.path.join(os.path.dirname(__file__),'references')
for name in ('man','woman'):
 bpy.ops.wm.read_factory_settings(use_empty=True)
 bpy.context.scene.render.fps=30
 bpy.ops.import_scene.gltf(filepath=os.path.join(folder,name+'.glb'))
 rig=next(o for o in bpy.data.objects if o.type=='ARMATURE')
 rig.animation_data.action=None
 for track in list(rig.animation_data.nla_tracks):rig.animation_data.nla_tracks.remove(track)
 for action in list(bpy.data.actions):bpy.data.actions.remove(action)
 for bone in rig.pose.bones:
  bone.rotation_mode='QUATERNION';bone.rotation_quaternion=Quaternion();bone.location=(0,0,0)
 bpy.context.view_layer.update()
 rest=rig.pose.bones['pelvis'].head.copy()
 poses,hips,metre=capture(rig,reference,'Strafe',range(1,34),Quaternion())
 mean=sum(hips,Vector())/len(hips)
 action=bpy.data.actions.new('Weapon_StrafeLeft');action.use_fake_user=True
 rig.animation_data.action=action
 for frame,(pose,hip) in enumerate(zip(poses,hips),1):
  for bone in rig.pose.bones:bone.rotation_quaternion=Quaternion();bone.location=(0,0,0)
  bpy.context.view_layer.update()
  for bone,q in pose.items():orient(rig.pose.bones[bone],q)
  pelvis=rig.pose.bones['pelvis']
  pelvis.location+=pelvis.bone.matrix_local.to_quaternion().inverted()@(rest+hip-mean-pelvis.head)
  bpy.context.view_layer.update()
  for bone in rig.pose.bones:
   bone.keyframe_insert('rotation_quaternion',frame=frame)
   if bone.name=='pelvis':bone.keyframe_insert('location',frame=frame)
 if action.slots:
  for curve in action.layers[0].strips[0].channelbag(action.slots[0]).fcurves:
   for key in curve.keyframe_points:key.interpolation='LINEAR'
 rig.animation_data.action=None
 mirror_clip(rig,'Weapon_StrafeLeft','Weapon_StrafeRight',30)
 bpy.ops.export_scene.gltf(filepath=os.path.join(out,name+'-weapon.glb'),export_format='GLB',export_animations=True,export_animation_mode='ACTIONS',export_morph=False)
