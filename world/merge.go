package world

import (
	"cmp"
	"fmt"
	"image/color"
	"math"
	"slices"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Merging. The engine draws each model on its own, a draw for each of its
// meshes, and a town is thousands of small pieces: deck plates, catwalks,
// rocks, crates. Drawn one at a time, they cost more in draws than in
// anything they draw, and twice, as the shadow map draws them again. So
// PlaceMerged merges the small pieces of a layout by where they stand, in
// squares mergeCell across:
//
//   - every copy of a piece in a square, into one mesh (each piece is
//     painted on its own texture, so only copies can share a draw), and
//     what glows on them into another;
//   - and the shadow every opaque piece in a square casts, into one mesh
//     of their shapes alone (the shadow map needs no paint).
//
// What a piece collides with, its ladders and its lights are placed as
// Place places them. Pieces of more than mergeVertices vertices, or reaching
// further than mergeRadius from their middle, are drawn on their own, as
// are vehicles: they're few, and the big ones are seen from far off.
const (
	mergeCell     = 32
	mergeVertices = 5000
	mergeRadius   = 8
	// meshMost is the most vertices one mesh can have: its triangles index
	// them in 16 bits.
	meshMost = math.MaxUint16
)

// Merged marks a mesh merged from pieces (see PlaceMerged). Center and
// Radius bound it, in its own frame. Piece is the radius of the biggest
// piece in it: how far off it's seen is how far off they would be. Shadow
// is whether it's only the shadow they cast (render.ShadowOnly); Casts,
// whether it casts its own (if not, its pieces' shadow is Shadow's).
type Merged struct {
	Center rl.Vector3
	Radius float32
	Piece  float32
	Shadow bool
	Casts  bool
}

// Stores are the asset stores PlaceMerged reads the pieces' models from and
// adds its meshes and their materials to.
type Stores struct {
	Models    *asset.Assets[render.Model]
	Meshes    *asset.Assets[render.Mesh]
	Materials *asset.Assets[render.StandardMaterial]
	Textures  *asset.Assets[render.Texture]
}

// look is how a merged mesh is painted: the texture and colour its pieces
// are, and what glows of them.
type look struct {
	texture  uint32
	color    color.RGBA
	emissive color.RGBA
}

type mergeKey struct {
	cx, cz int32
	look   look
}

// merging is a merged mesh being built: its meshes so far (each new one
// once the last is full), the texture it's painted with, the biggest
// piece in it, and whether it casts its own shadow.
type merging struct {
	meshes  []render.MeshData
	texture rl.Texture2D
	piece   float32
	casts   bool
}

// add appends a mesh's vertices and triangles, moved by place, to the last
// of the meshes (or a new one if it's full).
func (g *merging) add(src rl.Mesh, place func(rl.Vector3) rl.Vector3, turn rl.Quaternion, shape bool) {
	n := int(src.VertexCount)
	if n == 0 || n > meshMost {
		return
	}
	if len(g.meshes) == 0 || len(g.meshes[len(g.meshes)-1].Positions)+n > meshMost {
		g.meshes = append(g.meshes, render.MeshData{})
	}
	d := &g.meshes[len(g.meshes)-1]
	first := len(d.Positions)
	positions := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(src.Vertices)), n)
	for _, v := range positions {
		d.Positions = append(d.Positions, place(v))
	}
	if !shape {
		if src.Normals != nil {
			for _, v := range unsafe.Slice((*rl.Vector3)(unsafe.Pointer(src.Normals)), n) {
				d.Normals = append(d.Normals, rl.Vector3RotateByQuaternion(v, turn))
			}
		} else {
			d.Normals = append(d.Normals, make([]rl.Vector3, n)...)
		}
		if src.Texcoords != nil {
			d.Texcoords = append(d.Texcoords, unsafe.Slice((*rl.Vector2)(unsafe.Pointer(src.Texcoords)), n)...)
		} else {
			d.Texcoords = append(d.Texcoords, make([]rl.Vector2, n)...)
		}
	}
	if src.Indices != nil {
		for _, i := range unsafe.Slice(src.Indices, 3*int(src.TriangleCount)) {
			d.Indices = append(d.Indices, uint16(first)+i)
		}
	} else {
		for i := range n {
			d.Indices = append(d.Indices, uint16(first+i))
		}
	}
}

// mergeable reports whether a model is small enough to merge, and its
// radius about its middle.
func mergeable(m rl.Model) (float32, bool) {
	total := 0
	lo, hi := rl.Vector3{X: math.MaxFloat32, Y: math.MaxFloat32, Z: math.MaxFloat32}, rl.Vector3{X: -math.MaxFloat32, Y: -math.MaxFloat32, Z: -math.MaxFloat32}
	for _, mesh := range m.GetMeshes() {
		n := int(mesh.VertexCount)
		total += n
		if mesh.Vertices == nil || mesh.BoneCount > 0 || total > mergeVertices {
			return 0, false
		}
		for _, v := range unsafe.Slice((*rl.Vector3)(unsafe.Pointer(mesh.Vertices)), n) {
			lo, hi = rl.Vector3Min(lo, v), rl.Vector3Max(hi, v)
		}
	}
	r := rl.Vector3Distance(lo, hi) / 2
	return r, total > 0 && r <= mergeRadius
}

// paint is how a model's material is painted: its texture, colour and
// glow (raylib's glTF loader sets the emission map's colour when the
// material has an emissive texture, as the render package reads it).
func paint(mat rl.Material) (rl.Texture2D, look) {
	if mat.Maps == nil {
		return rl.Texture2D{}, look{color: color.RGBA{255, 255, 255, 255}}
	}
	diffuse := mat.GetMap(rl.MapDiffuse)
	l := look{texture: diffuse.Texture.ID, color: diffuse.Color}
	if e := mat.GetMap(rl.MapEmission); e.Texture.ID != 0 {
		l.emissive = e.Color
	}
	return diffuse.Texture, l
}

// opaque reports whether a texture has no alpha to cut a shadow out by.
func opaque(t rl.Texture2D) bool {
	return t.ID == 0 || t.Format == rl.UncompressedR8g8b8 || t.Format == rl.UncompressedR5g6b5
}

// PlaceMerged places layout as Place places each piece, but with the small
// pieces' models merged (see Merged).
func (k *Kit) PlaceMerged(cmd *illusion.Commands, layout []Placement, s Stores) error {
	groups := map[mergeKey]*merging{}
	shadows := map[[2]int32]*merging{}
	radius := map[asset.Handle[render.Model]]float32{}
	small := map[asset.Handle[render.Model]]bool{}
	for _, p := range layout {
		piece, ok := k.Pieces[p.Piece]
		if !ok {
			return fmt.Errorf("world: no piece %q", p.Piece)
		}
		var model *render.Model
		if piece.Vehicle == nil && s.Models != nil {
			model = s.Models.Get(piece.model)
		}
		if model == nil {
			if err := k.Place(cmd, p); err != nil {
				return err
			}
			continue
		}
		if _, known := small[piece.model]; !known {
			radius[piece.model], small[piece.model] = mergeable(model.Model)
		}
		if !small[piece.model] {
			if err := k.Place(cmd, p); err != nil {
				return err
			}
			continue
		}
		at := vec(p.At)
		turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
		k.placeParts(cmd, piece, at, turn)
		cell := [2]int32{int32(math.Floor(float64(at.X / mergeCell))), int32(math.Floor(float64(at.Z / mergeCell)))}
		origin := cellOrigin(cell)
		place := func(v rl.Vector3) rl.Vector3 {
			return rl.Vector3Add(rl.Vector3Subtract(at, origin), rl.Vector3RotateByQuaternion(v, turn))
		}
		mats := model.GetMaterials()
		meshMaterial := unsafe.Slice(model.MeshMaterial, model.MeshCount)
		for i, mesh := range model.GetMeshes() {
			var mat rl.Material
			if j := int(meshMaterial[i]); j >= 0 && j < len(mats) {
				mat = mats[j]
			}
			texture, l := paint(mat)
			key := mergeKey{cell[0], cell[1], l}
			g := groups[key]
			if g == nil {
				g = &merging{texture: texture, casts: !opaque(texture)}
				groups[key] = g
			}
			g.piece = max(g.piece, radius[piece.model])
			g.add(mesh, place, turn, false)
			if opaque(texture) {
				sh := shadows[cell]
				if sh == nil {
					sh = &merging{}
					shadows[cell] = sh
				}
				sh.piece = max(sh.piece, radius[piece.model])
				sh.add(mesh, place, turn, true)
			}
		}
	}
	// Spawn them in a fixed order, so the world's the same each time.
	textures := map[uint32]asset.Handle[render.Texture]{}
	keys := make([]mergeKey, 0, len(groups))
	for key := range groups {
		keys = append(keys, key)
	}
	slices.SortFunc(keys, func(a, b mergeKey) int {
		return cmp.Or(cmp.Compare(a.cx, b.cx), cmp.Compare(a.cz, b.cz), cmp.Compare(a.look.texture, b.look.texture),
			cmp.Compare(a.look.emissive.R, b.look.emissive.R), cmp.Compare(a.look.emissive.G, b.look.emissive.G), cmp.Compare(a.look.emissive.B, b.look.emissive.B))
	})
	for _, key := range keys {
		g := groups[key]
		th, ok := textures[key.look.texture]
		if !ok && g.texture.ID != 0 {
			// The piece's own texture, shared: the model it's loaded with
			// owns it.
			th = s.Textures.Add(render.Texture{Texture2D: g.texture})
			textures[key.look.texture] = th
		}
		mat := s.Materials.Add(render.StandardMaterial{BaseColor: key.look.color, Texture: th, Emissive: key.look.emissive})
		for _, d := range g.meshes {
			parts := []illusion.Component{
				illusion.C(render.Mesh3d{Mesh: s.Meshes.Add(render.Mesh{Mesh: render.NewMesh(d)})}),
				illusion.C(render.MeshMaterial3d{Material: mat}),
				illusion.C(transform.FromTranslation(cellOrigin([2]int32{key.cx, key.cz}))),
				illusion.C(merged(d, g.piece, false, g.casts)),
			}
			if !g.casts {
				parts = append(parts, illusion.C(render.NotShadowCaster{}))
			}
			cmd.Spawn(parts...)
		}
	}
	cells := make([][2]int32, 0, len(shadows))
	for cell := range shadows {
		cells = append(cells, cell)
	}
	slices.SortFunc(cells, func(a, b [2]int32) int { return cmp.Or(cmp.Compare(a[0], b[0]), cmp.Compare(a[1], b[1])) })
	for _, cell := range cells {
		sh := shadows[cell]
		for _, d := range sh.meshes {
			cmd.Spawn(
				illusion.C(render.Mesh3d{Mesh: s.Meshes.Add(render.Mesh{Mesh: render.NewMesh(d)})}),
				illusion.C(transform.FromTranslation(cellOrigin(cell))),
				illusion.C(merged(d, sh.piece, true, true)),
				illusion.C(render.ShadowOnly{}),
			)
		}
	}
	return nil
}

// cellOrigin is the middle of a square pieces are merged by, on the ground.
func cellOrigin(cell [2]int32) rl.Vector3 {
	return rl.Vector3{X: (float32(cell[0]) + .5) * mergeCell, Z: (float32(cell[1]) + .5) * mergeCell}
}

// merged is the Merged of a mesh of d.
func merged(d render.MeshData, piece float32, shadow, casts bool) Merged {
	lo, hi := d.Positions[0], d.Positions[0]
	for _, v := range d.Positions {
		lo, hi = rl.Vector3Min(lo, v), rl.Vector3Max(hi, v)
	}
	return Merged{
		Center: rl.Vector3Scale(rl.Vector3Add(lo, hi), .5),
		Radius: rl.Vector3Distance(lo, hi) / 2,
		Piece:  piece,
		Shadow: shadow,
		Casts:  casts,
	}
}
