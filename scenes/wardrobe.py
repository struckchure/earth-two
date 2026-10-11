"""Preview the actual game wardrobe, with its existing rigs and skin patches."""
import json
from pathlib import Path
import bpy
from harness_blender import fresh, parameters, frame_all, export_glb, render, turntable, report

p=parameters({'faction':{'type':'choice','default':'Crew','options':['Crew','Charter','Registrar','Fringer','Corvane']}},title='Colony wardrobe',sources=['scenes/wardrobe.py','assets/characters'],output='out/model.glb')
fresh(scale=1.0)
wardrobe=json.loads(Path('assets/characters/wardrobe.json').read_text())
for index,body in enumerate(wardrobe['bodies']):
    look=next(l for l in body['looks'] if l['name']==p['faction'])
    worn=[next(i for i in body['slots'][s] if i['name']==name) for s,name in look['wear'].items() if name]
    hidden=set()
    for item in worn:
        for region in item['hides']: hidden.update(body['regions'].get(region,[]))
        for region in item['covers']: hidden.update(body['underwear'].get(region,[]))
    if look['wear'].get('top') or look['wear'].get('outfit'): hidden.update(body['underwear'].get('torso',[]))
    collection=bpy.data.collections.new(body['name'].title()+' / '+p['faction']); bpy.context.scene.collection.children.link(collection)
    for path,is_body in [(body['model'],True)]+[(i['path'],False) for i in worn]+[(body['slots']['hair'][0]['path'],False)]:
        before=set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=str(Path('assets')/path))
        added=set(bpy.data.objects)-before
        for o in list(added):
            if o.name.startswith("Icosphere"):
                added.remove(o); bpy.data.objects.remove(o,do_unlink=True)
        if is_body:
            import struct
            raw=(Path('assets')/path).read_bytes(); n=struct.unpack_from('<I',raw,12)[0]; doc=json.loads(raw[20:20+n])
            names={doc['meshes'][i]['name'] for i in hidden}
            for o in list(added):
                if o.type=='MESH' and (o.data.name in names or o.name in names):
                    added.remove(o); bpy.data.objects.remove(o,do_unlink=True)
        for o in added:
            if o.parent is None: o.location.x+=(index-.5)*1.15
            for c in list(o.users_collection): c.objects.unlink(o)
            collection.objects.link(o)
            o.name=body["name"]+" / "+path+" / "+o.name
            if o.type=="MESH": o.data.name=o.name
    # Importers retain skin materials and armature bindings unchanged.
for o in bpy.data.objects:
    o.animation_data_clear()
    if o.type=="ARMATURE":
        for bone in o.pose.bones: bone.matrix_basis.identity()
for action in list(bpy.data.actions):
    bpy.data.actions.remove(action)
frame_all(azimuth=-90,elevation=8)
export_glb('out/model.glb')
render('out/preview.png',engine='CYCLES',samples=24)
try:
    turntable('out/turntable.mp4',seconds=3,fps=12,size=(640,640))

finally:
    report()
