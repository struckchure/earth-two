"""Append Blender-authored animation data while preserving the original GLB.
Usage: python3 tools/makehuman/merge_animations.py assets/characters build/traversal
"""
import copy
import json
import pathlib
import struct
import sys

# The clips traversal.py mirrors from the rig's own (its MIRRORED).
MIRRORED={'Punching Mirrored'}


def authored(name):
    return name.startswith('Traversal_') or name in MIRRORED


def read(path):
    raw=path.read_bytes();assert raw[:4]==b'glTF'
    offset=12;doc=None;data=b''
    while offset<len(raw):
        size,kind=struct.unpack_from('<II',raw,offset);chunk=raw[offset+8:offset+8+size];offset+=8+size
        if kind==0x4e4f534a:doc=json.loads(chunk)
        elif kind==0x004e4942:data=chunk
    return doc,data


def merge(path,donor):
    doc,data=read(path);src,buf=read(donor)
    extras=doc.setdefault('extras',{})
    baseline=extras.get('earthTwoTraversalBase')
    if baseline is None:
        baseline={'accessors':len(doc['accessors']),'views':len(doc['bufferViews']),'bytes':len(data)}
    extras['earthTwoTraversalBase']=baseline
    doc['accessors']=doc['accessors'][:baseline['accessors']]
    doc['bufferViews']=doc['bufferViews'][:baseline['views']]
    data=data[:baseline['bytes']]
    names={n.get('name'):i for i,n in enumerate(doc['nodes'])}
    mapping={i:names[n['name']] for i,n in enumerate(src['nodes']) if n.get('name') in names}
    selected=[copy.deepcopy(a) for a in src.get('animations',[]) if authored(a.get('name',''))]
    required={'Traversal_'+n for n in ('Slide','Ladder','Vault','Mantle','WallKick','WallKickRight','WallKickFall','WallLand','WallKickFallRight','Crouch','StandUp','LadderExit','Fall','StairsUp')}|MIRRORED
    assert {a['name'] for a in selected} == required, 'missing or unexpected authored clips'
    # Copy only the animation accessors/views, without donor meshes or textures.
    accessors={};views={};binary=bytearray(data)
    def accessor(index):
        if index in accessors:return accessors[index]
        a=copy.deepcopy(src['accessors'][index]);v=a['bufferView']
        assert 'sparse' not in a
        if v not in views:
            view=copy.deepcopy(src['bufferViews'][v]);assert view.get('buffer',0)==0
            start=view.get('byteOffset',0);chunk=buf[start:start+view['byteLength']]
            binary.extend(b'\0'*(-len(binary)%4));view['byteOffset']=len(binary);view['buffer']=0
            views[v]=len(doc['bufferViews']);doc['bufferViews'].append(view);binary.extend(chunk)
        a['bufferView']=views[v];accessors[index]=len(doc['accessors']);doc['accessors'].append(a)
        return accessors[index]
    for animation in selected:
        for channel in animation['channels']:
            channel['target']['node']=mapping[channel['target']['node']]
        for sampler in animation['samplers']:
            sampler['input']=accessor(sampler['input']);sampler['output']=accessor(sampler['output'])
    doc['animations']=[a for a in doc['animations'] if not authored(a.get('name',''))]+selected
    binary.extend(b'\0'*(-len(binary)%4));doc['buffers']=[{'byteLength':len(binary)}]
    js=json.dumps(doc,separators=(',',':')).encode();js+=b' '*(-len(js)%4)
    result=struct.pack('<III',0x46546c67,2,28+len(js)+len(binary))+struct.pack('<II',len(js),0x4e4f534a)+js+struct.pack('<II',len(binary),0x004e4942)+binary
    path.write_bytes(result)
    print(path, 'added', ', '.join(a['name'] for a in selected))

if __name__=='__main__':
    folder,donors=map(pathlib.Path,sys.argv[1:3])
    for name in ('man','woman'):merge(folder/(name+'.glb'),donors/(name+'-traversal.glb'))
