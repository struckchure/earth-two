package character

import (
	"math"
	"slices"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
)

// Hair and loose clothes move with physics (render.Cloth): they swing and
// trail as the character moves, hang under gravity, and are pushed out of
// the body. What sits against the skin stays where the animation puts it;
// the further a part is, along the garment, from anything touching the
// skin, the further it can stray. Glasses and shoes are rigid.

// clothKind is how one slot's items move.
type clothKind struct {
	// Touching is how close to the skin a vertex is held.
	Touching float32
	// Rate is how much freedom a vertex gains per metre along the garment
	// from the nearest held one, up to Most metres.
	Rate, Most float32
	// Stiffness is render.Cloth's: how hard it's pulled back into shape.
	Stiffness float32
}

var clothKinds = map[Slot]clothKind{
	Hair:     {Touching: 0.025, Rate: 0.5, Most: 0.15, Stiffness: 0.04},
	Top:      {Touching: 0.02, Rate: 0.3, Most: 0.08, Stiffness: 0.06},
	Bottom:   {Touching: 0.02, Rate: 0.3, Most: 0.08, Stiffness: 0.06},
	OnePiece: {Touching: 0.02, Rate: 0.3, Most: 0.08, Stiffness: 0.06},
}

// bodyCapsules are the body's colliders: from one bone's origin to
// another's, fitted to the vertices the listed bones move most. The trunk,
// wider than it's deep, gets two side by side (split), one each side.
var bodyCapsules = []struct {
	from, to string
	bones    []string
	split    bool
}{
	{"pelvis", "spine_02", []string{"pelvis", "spine_01"}, true},
	{"spine_02", "neck_01", []string{"spine_02", "spine_03"}, true},
	{"neck_01", "head", []string{"neck_01"}, false},
	{"clavicle_l", "upperarm_l", []string{"clavicle_l"}, false},
	{"clavicle_r", "upperarm_r", []string{"clavicle_r"}, false},
	{"upperarm_l", "lowerarm_l", []string{"upperarm_l"}, false},
	{"upperarm_r", "lowerarm_r", []string{"upperarm_r"}, false},
	{"lowerarm_l", "hand_l", []string{"lowerarm_l"}, false},
	{"lowerarm_r", "hand_r", []string{"lowerarm_r"}, false},
	{"thigh_l", "calf_l", []string{"thigh_l"}, false},
	{"thigh_r", "calf_r", []string{"thigh_r"}, false},
	{"calf_l", "foot_l", []string{"calf_l"}, false},
	{"calf_r", "foot_r", []string{"calf_r"}, false},
}

// capsuleFit is the share of a capsule's vertices it reaches out to: just
// under the skin, so cloth kept out of it (with its thickness) sits on the
// body rather than off it.
const capsuleFit = 0.4

// clothed marks a garment that has its Cloth, or needs none.
type clothed struct{}

// clothCache keeps what clothe works out per model.
type clothCache struct {
	fits   map[clothKey]map[int]render.ClothMesh
	bodies map[asset.Handle[render.Model]]*bodyShape
}

type clothKey struct {
	garment, body asset.Handle[render.Model]
	slot          Slot
}

// bodyShape is a body model as clothe needs it: its colliders, and its
// vertices in a grid, to find what's near the skin.
type bodyShape struct {
	capsules []render.Capsule
	grid     map[[3]int32][]rl.Vector3
	cell     float32
}

// clothe gives each new garment a Cloth fitted to the body it's on.
func clothe(
	cmd *illusion.Commands,
	garments *illusion.Query2Where[Garment, render.Model3d, illusion.Without[clothed]],
	models3d *illusion.Query1[render.Model3d],
	hier *illusion.Hierarchy,
	models *illusion.Res[asset.Assets[render.Model]],
	cache *illusion.Local[clothCache],
) {
	c := cache.Get()
	if c.fits == nil {
		c.fits = map[clothKey]map[int]render.ClothMesh{}
		c.bodies = map[asset.Handle[render.Model]]*bodyShape{}
	}
	store := models.Get()
	type job struct {
		e     ecs.Entity
		cloth *render.Cloth
	}
	var done []job
	garments.Each(func(e ecs.Entity, g *Garment, m *render.Model3d) {
		kind, moves := clothKinds[g.Slot]
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		pm, ok := models3d.Get(parent)
		if !ok {
			return
		}
		garment, body := store.Get(m.Model), store.Get(pm.Model)
		if garment == nil || body == nil {
			return // not loaded yet
		}
		if !moves {
			done = append(done, job{e, nil})
			return
		}
		shape := c.bodies[pm.Model]
		if shape == nil {
			shape = newBodyShape(body.Model)
			c.bodies[pm.Model] = shape
		}
		key := clothKey{m.Model, pm.Model, g.Slot}
		fit, ok := c.fits[key]
		if !ok {
			fit = fitCloth(garment.Model, shape, kind, g.skin)
			c.fits[key] = fit
		}
		if len(fit) == 0 {
			done = append(done, job{e, nil})
			return
		}
		done = append(done, job{e, &render.Cloth{Meshes: fit, Colliders: shape.capsules, Stiffness: kind.Stiffness}})
	})
	for _, j := range done {
		if j.cloth != nil {
			cmd.Entity(j.e).Insert(illusion.C(clothed{}), illusion.C(*j.cloth))
		} else {
			cmd.Entity(j.e).Insert(illusion.C(clothed{}))
		}
	}
}

// fitCloth works out how each of garment's meshes moves on body; meshes
// that don't move (and the skin patches, skin) are left out.
func fitCloth(garment rl.Model, body *bodyShape, kind clothKind, skin []int) map[int]render.ClothMesh {
	out := map[int]render.ClothMesh{}
	for i, m := range garment.GetMeshes() {
		if slices.Contains(skin, i) || m.BoneWeights == nil || m.VertexCount == 0 {
			continue
		}
		vertices := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(m.Vertices)), m.VertexCount)
		pinned := make([]bool, len(vertices))
		for v, p := range vertices {
			pinned[v] = body.near(p, kind.Touching)
		}
		freedom := render.ClothFreedom(vertices, triangles(m), pinned, kind.Rate, kind.Most)
		if slices.ContainsFunc(freedom, func(f float32) bool { return f > 0 }) {
			out[i] = render.ClothMesh{Freedom: freedom}
		}
	}
	return out
}

func triangles(m rl.Mesh) []int32 {
	n := 3 * int(m.TriangleCount)
	out := make([]int32, n)
	if m.Indices == nil {
		for i := range out {
			out[i] = int32(i)
		}
		return out
	}
	for i, v := range unsafe.Slice(m.Indices, n) {
		out[i] = int32(v)
	}
	return out
}

// newBodyShape reads body's bind pose: its vertices, and each one's bone.
func newBodyShape(body rl.Model) *bodyShape {
	s := &bodyShape{grid: map[[3]int32][]rl.Vector3{}, cell: 0.03}
	byBone := map[int][]rl.Vector3{}
	for _, m := range body.GetMeshes() {
		if m.VertexCount == 0 || m.Vertices == nil {
			continue
		}
		n := int(m.VertexCount)
		vertices := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(m.Vertices)), n)
		var ids []uint8
		var weights []float32
		if m.BoneIndices != nil && m.BoneWeights != nil {
			ids, weights = unsafe.Slice(m.BoneIndices, 4*n), unsafe.Slice(m.BoneWeights, 4*n)
		}
		for v, p := range vertices {
			k := s.key(p)
			s.grid[k] = append(s.grid[k], p)
			if ids != nil {
				best := 0
				for j := 1; j < 4; j++ {
					if weights[4*v+j] > weights[4*v+best] {
						best = j
					}
				}
				byBone[int(ids[4*v+best])] = append(byBone[int(ids[4*v+best])], p)
			}
		}
	}
	s.capsules = fitCapsules(body, byBone)
	return s
}

func (s *bodyShape) key(p rl.Vector3) [3]int32 {
	return [3]int32{int32(math.Floor(float64(p.X / s.cell))), int32(math.Floor(float64(p.Y / s.cell))), int32(math.Floor(float64(p.Z / s.cell)))}
}

// near reports whether a body vertex is within d (at most the grid's cell)
// of p.
func (s *bodyShape) near(p rl.Vector3, d float32) bool {
	k := s.key(p)
	for x := k[0] - 1; x <= k[0]+1; x++ {
		for y := k[1] - 1; y <= k[1]+1; y++ {
			for z := k[2] - 1; z <= k[2]+1; z++ {
				for _, q := range s.grid[[3]int32{x, y, z}] {
					if rl.Vector3Distance(p, q) < d {
						return true
					}
				}
			}
		}
	}
	return false
}

// fitCapsules fits bodyCapsules, and a sphere for the head, to the body's
// vertices by bone.
func fitCapsules(body rl.Model, byBone map[int][]rl.Vector3) []render.Capsule {
	bone := map[string]int{}
	for i, b := range body.Skeleton.GetBones() {
		bone[boneName(b)] = i
	}
	bind := body.Skeleton.GetBindPose()
	var out []render.Capsule
	for _, c := range bodyCapsules {
		from, ok1 := bone[c.from]
		to, ok2 := bone[c.to]
		if !ok1 || !ok2 {
			continue
		}
		var points []rl.Vector3
		for _, name := range c.bones {
			if i, ok := bone[name]; ok {
				points = append(points, byBone[i]...)
			}
		}
		groups := [][]rl.Vector3{points}
		if c.split {
			groups = splitSides(points, bind[from].Translation.X)
		}
		for _, g := range groups {
			if k, ok := fitCapsule(bind[from].Translation, bind[to].Translation, g); ok {
				k.BoneA, k.BoneB = from, to
				out = append(out, k)
			}
		}
	}
	if head, ok := bone["head"]; ok {
		if pts := byBone[head]; len(pts) > 0 {
			var centre rl.Vector3
			for _, p := range pts {
				centre = rl.Vector3Add(centre, p)
			}
			centre = rl.Vector3Scale(centre, 1/float32(len(pts)))
			// Upright: a head is taller than it is wide.
			a, b := rl.Vector3Add(centre, rl.Vector3{Y: -0.03}), rl.Vector3Add(centre, rl.Vector3{Y: 0.03})
			if k, ok := fitCapsule(a, b, pts); ok {
				k.BoneA, k.BoneB = head, head
				out = append(out, k)
			}
		}
	}
	return out
}

// splitSides splits points into those left and right of x (the body
// faces +Z in its bind pose, so X is across it).
func splitSides(points []rl.Vector3, x float32) [][]rl.Vector3 {
	var left, right []rl.Vector3
	for _, p := range points {
		if p.X < x {
			left = append(left, p)
		} else {
			right = append(right, p)
		}
	}
	return [][]rl.Vector3{left, right}
}

// fitCapsule centres the segment a-b among points (bones run nearer the
// back than the middle) and gives it the radius reaching capsuleFit of
// them.
func fitCapsule(a, b rl.Vector3, points []rl.Vector3) (render.Capsule, bool) {
	if len(points) == 0 {
		return render.Capsule{}, false
	}
	var off rl.Vector3
	for _, p := range points {
		off = rl.Vector3Add(off, rl.Vector3Subtract(p, closestOnSegment(p, a, b)))
	}
	off = rl.Vector3Scale(off, 1/float32(len(points)))
	a, b = rl.Vector3Add(a, off), rl.Vector3Add(b, off)
	d := make([]float32, len(points))
	for i, p := range points {
		d[i] = rl.Vector3Distance(p, closestOnSegment(p, a, b))
	}
	slices.Sort(d)
	return render.Capsule{A: a, B: b, Radius: d[int(capsuleFit*float32(len(d)-1))]}, true
}

func closestOnSegment(p, a, b rl.Vector3) rl.Vector3 {
	ab := rl.Vector3Subtract(b, a)
	l := rl.Vector3DotProduct(ab, ab)
	if l < 1e-12 {
		return a
	}
	t := rl.Vector3DotProduct(rl.Vector3Subtract(p, a), ab) / l
	return rl.Vector3Add(a, rl.Vector3Scale(ab, max(0, min(1, t))))
}

func boneName(b rl.BoneInfo) string {
	n := make([]byte, 0, len(b.Name))
	for _, c := range b.Name {
		if c == 0 {
			break
		}
		n = append(n, byte(c))
	}
	return string(n)
}
