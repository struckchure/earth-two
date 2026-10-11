"""Retrofit garment textures without changing rig, mesh order or coverage.

Run with Blender's Python: scifi.py assets/characters. Idempotent per GLB;
wardrobe.py also applies this after each fresh garment export.
"""
import io
import json
import struct
import sys
from pathlib import Path
import numpy as np
import bpy
import tempfile

SLOTS = {'top', 'bottom', 'outfit', 'coat', 'shoes', 'mask'}

def read(path):
    raw = Path(path).read_bytes()
    size = struct.unpack_from('<I', raw, 12)[0]
    doc = json.loads(raw[20:20+size])
    return doc, raw[28+size:]

def accessor(doc, binary, index):
    a = doc['accessors'][index]
    v = doc['bufferViews'][a['bufferView']]
    dtype = {5126:'<f4',5125:'<u4',5123:'<u2',5121:'u1'}[a['componentType']]
    cols = {'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}[a['type']]
    return np.ndarray((a['count'],cols), dtype=dtype, buffer=binary,
        offset=v.get('byteOffset',0)+a.get('byteOffset',0),
        strides=(v.get('byteStride',np.dtype(dtype).itemsize*cols),np.dtype(dtype).itemsize))

def retrofit(path, slot, label, skin_meshes=()):
    if slot not in SLOTS:
        return
    doc, binary = read(path)
    if doc.get('extras', {}).get('colonyFinish') == 1:
        return
    # Mutate only colour images, retaining every existing accessor and mesh index.
    base = np.array([.26,.34,.37]); accent = np.array([.38,.78,.78])
    if 'Crew' in label or label == 'Overalls': base,accent = np.array([.72,.30,.09]),np.array([.85,.79,.58])
    elif 'Charter' in label: base,accent=np.array([.10,.17,.29]),np.array([.82,.87,.84])
    elif 'Registrar' in label or 'Grey sweater' in label: base,accent=np.array([.12,.15,.18]),np.array([.65,.48,.23])
    elif 'poncho' in label or slot=='mask': base,accent=np.array([.52,.43,.31]),np.array([.78,.61,.32])
    elif 'Suit' in label or 'Blouse' in label: base,accent=np.array([.23,.25,.29]),np.array([.68,.76,.79])
    images = {}
    for mi, mesh in enumerate(doc['meshes']):
        if mi in skin_meshes: continue
        for prim in mesh['primitives']:
            mat = doc['materials'][prim['material']]
            pbr = mat.get('pbrMetallicRoughness',{})
            if 'baseColorTexture' not in pbr: continue
            source=doc['textures'][pbr['baseColorTexture']['index']]['source']
            if source not in images:
                view=doc['bufferViews'][doc['images'][source]['bufferView']]
                with tempfile.NamedTemporaryFile(suffix='.png') as tmp:
                    tmp.write(binary[view.get('byteOffset',0):view.get('byteOffset',0)+view['byteLength']]); tmp.flush()
                    image=bpy.data.images.load(tmp.name, check_existing=False)
                    iw,ih=image.size
                    pixels=np.empty(iw*ih*4,dtype=np.float32); image.pixels.foreach_get(pixels)
                    old=np.clip(pixels.reshape(ih,iw,4)[::-1]*255,0,255).astype('uint8')
                    bpy.data.images.remove(image)
                h,w=old.shape[:2]
                lum=old[:,:,:3].mean(axis=2)/255
                shade=.62+.38*lum
                rgb=base[None,None,:]*shade[:,:,None]
                images[source]=(old,rgb,shade)
            old,rgb,shade=images[source]; h,w=old.shape[:2]
            pos=accessor(doc,binary,prim['attributes']['POSITION'])
            uv=accessor(doc,binary,prim['attributes']['TEXCOORD_0'])*[w-1,h-1]
            tris=accessor(doc,binary,prim['indices']).reshape(-1,3)
            low=pos.min(axis=0); extent=np.maximum(pos.max(axis=0)-low,1e-6)
            for tri in tris:
                t=uv[tri]; x0,y0=np.maximum(np.floor(t.min(axis=0)).astype(int),0); x1,y1=np.minimum(np.ceil(t.max(axis=0)).astype(int),[w-1,h-1])
                if x1<x0 or y1<y0: continue
                xx,yy=np.meshgrid(np.arange(x0,x1+1),np.arange(y0,y1+1))
                a,b,c=t; den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
                if abs(den)<1e-8: continue
                u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den
                v=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den
                inside=(u>=-.015)&(v>=-.015)&(u+v<=1.015)
                q=(u[:,:,None]*pos[tri[0]]+v[:,:,None]*pos[tri[1]]+(1-u-v)[:,:,None]*pos[tri[2]]-low)/extent
                x,y,z=q[:,:,0],q[:,:,1],q[:,:,2]
                # glTF garment coordinates are Y-up. Bands wrap in 3D, across UV seams.
                panel=((y>.65)&(y<.89)&(abs(x-.5)<.29))|((y>.20)&(y<.39))
                if slot in {'bottom','shoes'}: panel=((y>.22)&(y<.43))|(y<.12)
                band=(abs(y-.67)<.012)|(abs(y-.24)<.009)
                piping=(abs(abs(x-.5)-.29)<.008)&(y>.42)&(y<.88)
                badge=(x>.56)&(x<.66)&(y>.76)&(y<.80)
                ticks=badge & ((x*160).astype(int)%3!=0)
                colors=np.broadcast_to(base,q.shape).copy()
                colors[panel]=[.075,.10,.12]
                colors[band|piping|ticks]=accent
                # Restrained fine ripstop weave; original shading retains folds and wear.
                weave=1-.045*((xx%6==0)|(yy%6==0))
                patch=colors*shade[y0:y1+1,x0:x1+1,None]*weave[:,:,None]
                rgb[y0:y1+1,x0:x1+1][inside]=patch[inside]
            pbr['roughnessFactor']=.78
            pbr['metallicFactor']=0
    data=bytearray(binary)
    for source,(old,rgb,_) in images.items():
        old[:,:,:3]=np.clip(rgb*255,0,255).astype('uint8')
        image=bpy.data.images.new('Colony textile',width=old.shape[1],height=old.shape[0],alpha=True)
        image.pixels.foreach_set((old[::-1].astype(np.float32)/255).ravel())
        with tempfile.NamedTemporaryFile(suffix='.png') as tmp:
            image.filepath_raw=tmp.name; image.file_format='PNG'; image.save()
            stream=io.BytesIO(Path(tmp.name).read_bytes())
        bpy.data.images.remove(image)
        data.extend(b'\0'*(-len(data)%4)); off=len(data); data.extend(stream.getvalue())
        doc['bufferViews'].append({'buffer':0,'byteOffset':off,'byteLength':len(stream.getvalue())})
        doc['images'][source]={'bufferView':len(doc['bufferViews'])-1,'mimeType':'image/png','name':label+' colony textile'}
    doc.setdefault('extras',{})['colonyFinish']=1
    doc['buffers'][0]['byteLength']=len(data)
    data.extend(b'\0'*(-len(data)%4))
    header=json.dumps(doc,separators=(',',':')).encode(); header+=b' '*(-len(header)%4)
    Path(path).write_bytes(struct.pack('<III',0x46546c67,2,28+len(header)+len(data))+struct.pack('<II',len(header),0x4e4f534a)+header+struct.pack('<II',len(data),0x004e4942)+data)

if __name__=='__main__':
    root=Path(sys.argv[-1]); wardrobe=json.loads((root/'wardrobe.json').read_text())
    for body in wardrobe['bodies']:
        for slot,items in body['slots'].items():
            for item in items:
                retrofit(root.parent/item['path'],slot,item['name'],item['skinMeshes'])
                print(body['name'],slot,item['name'],flush=True)
