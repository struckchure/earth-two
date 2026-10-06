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
	Stiffness        float32
	Damping, Bending float32
	// Support holds shoulder straps and necklines to their animated pose.
	// It is the depth below the highest vertex, in metres; zero disables it.
	Support float32
}

var clothKinds = map[Slot]clothKind{
	Hair:     {Touching: 0.025, Rate: 0.5, Most: 0.15, Stiffness: 0.035, Damping: 0.08, Bending: 0.35},
	Top:      {Touching: 0.02, Rate: 0.3, Most: 0.08, Stiffness: 0.045, Damping: 0.12, Bending: 0.6, Support: 0.10},
	Bottom:   {Touching: 0.02, Rate: 0.25, Most: 0.05, Stiffness: 0.06, Damping: 0.16, Bending: 0.75},
	OnePiece: {Touching: 0.02, Rate: 0.3, Most: 0.08, Stiffness: 0.045, Damping: 0.12, Bending: 0.6, Support: 0.10},
	// A coat hangs looser than what's under it, and swings further.
	Coat: {Touching: 0.03, Rate: 0.3, Most: 0.12, Stiffness: 0.04, Damping: 0.12, Bending: 0.55, Support: 0.10},
}

// bodyCapsules are the body's colliders: from one bone's origin to
// another's, fitted to the vertices the listed bones move most. The trunk,
// wider than it's deep, gets two side by side (split), one each side. Each
// is cut into pieces along its length, each fitted to its own stretch of
// the body: a limb narrows along its length (a thigh at the knee is much
// thinner than at the hip), and one capsule would be its widest part's
// size all the way, pushing loose clothes out where it's thin.
var bodyCapsules = []struct {
	from, to string
	bones    []string
	split    bool
	pieces   int
}{
	{"pelvis", "spine_02", []string{"pelvis", "spine_01"}, true, 3},
	{"spine_02", "neck_01", []string{"spine_02", "spine_03"}, true, 3},
	{"neck_01", "head", []string{"neck_01"}, false, 1},
	{"clavicle_l", "upperarm_l", []string{"clavicle_l"}, false, 1},
	{"clavicle_r", "upperarm_r", []string{"clavicle_r"}, false, 1},
	{"upperarm_l", "lowerarm_l", []string{"upperarm_l"}, false, 3},
	{"upperarm_r", "lowerarm_r", []string{"upperarm_r"}, false, 3},
	{"lowerarm_l", "hand_l", []string{"lowerarm_l"}, false, 3},
	{"lowerarm_r", "hand_r", []string{"lowerarm_r"}, false, 3},
	{"thigh_l", "calf_l", []string{"thigh_l"}, false, 4},
	{"thigh_r", "calf_r", []string{"thigh_r"}, false, 4},
	{"calf_l", "foot_l", []string{"calf_l"}, false, 3},
	{"calf_r", "foot_r", []string{"calf_r"}, false, 3},
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
	shoes  map[asset.Handle[render.Model]][]render.Capsule
}

type clothKey struct {
	garment, body, footwear asset.Handle[render.Model]
	slot                    Slot
}

// bodyShape is a body model as clothe needs it: its colliders, and its
// vertices in a grid, to find what's near the skin.
type bodyShape struct {
	capsules []render.Capsule
	grid     map[[3]int32][]rl.Vector3
	cell     float32
	surface  map[[3]int32][][3]rl.Vector3
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
		c.shoes = map[asset.Handle[render.Model]][]render.Capsule{}
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
		var shoeCaps []render.Capsule
		if !g.footwear.IsZero() {
			shoe, loaded := store.Get(g.footwear), false
			if shoe == nil {
				return
			}
			shoeCaps, loaded = c.shoes[g.footwear]
			if !loaded {
				shoeCaps = footwearCapsules(shoe.Model)
				c.shoes[g.footwear] = shoeCaps
			}
		}
		key := clothKey{m.Model, pm.Model, g.footwear, g.Slot}
		fit, ok := c.fits[key]
		if !ok {
			fit = fitCloth(garment.Model, shape, kind, g.skin, shoeCaps)
			c.fits[key] = fit
		}
		if len(fit) == 0 {
			done = append(done, job{e, nil})
			return
		}
		colliders := append(slices.Clone(shape.capsules), shoeCaps...)
		done = append(done, job{e, &render.Cloth{Meshes: fit, Colliders: colliders, Stiffness: kind.Stiffness, Damping: kind.Damping, Bending: kind.Bending}})
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
func fitCloth(garment rl.Model, body *bodyShape, kind clothKind, skin []int, footwear []render.Capsule) map[int]render.ClothMesh {
	out := map[int]render.ClothMesh{}
	for i, m := range garment.GetMeshes() {
		if slices.Contains(skin, i) || m.BoneWeights == nil || m.VertexCount == 0 {
			continue
		}
		vertices := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(m.Vertices)), m.VertexCount)
		pinned := clothPins(vertices, body, kind)
		for v, p := range vertices {
			if cuffClearance(p, footwear) > 0 {
				pinned[v] = false
			}
		}
		freedom := render.ClothFreedom(vertices, triangles(m), pinned, kind.Rate, kind.Most)
		for v, p := range vertices {
			freedom[v] = max(freedom[v], cuffClearance(p, footwear))
		}
		if slices.ContainsFunc(freedom, func(f float32) bool { return f > 0 }) {
			out[i] = render.ClothMesh{Freedom: freedom}
		}
	}
	return out
}

// The shoulders support a top even when its straps stand further off the
// skin than Touching. Letting those supports simulate lifts the neckline
// and makes the rest of the garment hang from moving, floating anchors.
func clothPins(vertices []rl.Vector3, body *bodyShape, kind clothKind) []bool {
	pinned := make([]bool, len(vertices))
	if len(vertices) == 0 {
		return pinned
	}
	top := vertices[0].Y
	for _, p := range vertices[1:] {
		top = max(top, p.Y)
	}
	for v, p := range vertices {
		pinned[v] = kind.Support > 0 && p.Y >= top-kind.Support || body.near(p, kind.Touching)
	}
	return pinned
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
	s := &bodyShape{grid: map[[3]int32][]rl.Vector3{}, cell: 0.03, surface: map[[3]int32][][3]rl.Vector3{}}
	byBone := map[int][]rl.Vector3{}
	for _, m := range body.GetMeshes() {
		if m.VertexCount == 0 || m.Vertices == nil {
			continue
		}
		n := int(m.VertexCount)
		vertices := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(m.Vertices)), n)
		indices := triangles(m)
		for i := 0; i+2 < len(indices); i += 3 {
			s.addSurface([3]rl.Vector3{vertices[indices[i]], vertices[indices[i+1]], vertices[indices[i+2]]})
		}
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

// near reports whether the body surface is within d of p. Testing faces
// also holds cloth over the centres of large body triangles, where no body
// vertex is nearby.
func (s *bodyShape) near(p rl.Vector3, d float32) bool {
	k := s.key(p)
	r := int32(math.Ceil(float64(d / s.cell)))
	for x := k[0] - r; x <= k[0]+r; x++ {
		for y := k[1] - r; y <= k[1]+r; y++ {
			for z := k[2] - r; z <= k[2]+r; z++ {
				for _, face := range s.surface[[3]int32{x, y, z}] {
					if pointTriangleDistance(p, face) < d {
						return true
					}
				}
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

// Index each face in the grid cells crossed by its bounding box.
func (s *bodyShape) addSurface(face [3]rl.Vector3) {
	lo, hi := s.key(face[0]), s.key(face[0])
	for _, p := range face[1:] {
		k := s.key(p)
		for axis := range 3 {
			lo[axis], hi[axis] = min(lo[axis], k[axis]), max(hi[axis], k[axis])
		}
	}
	for x := lo[0]; x <= hi[0]; x++ {
		for y := lo[1]; y <= hi[1]; y++ {
			for z := lo[2]; z <= hi[2]; z++ {
				k := [3]int32{x, y, z}
				s.surface[k] = append(s.surface[k], face)
			}
		}
	}
}

func pointTriangleDistance(p rl.Vector3, face [3]rl.Vector3) float32 {
	a, b, c := face[0], face[1], face[2]
	ab, ac, ap := rl.Vector3Subtract(b, a), rl.Vector3Subtract(c, a), rl.Vector3Subtract(p, a)
	d00, d01, d11 := rl.Vector3DotProduct(ab, ab), rl.Vector3DotProduct(ab, ac), rl.Vector3DotProduct(ac, ac)
	d20, d21 := rl.Vector3DotProduct(ap, ab), rl.Vector3DotProduct(ap, ac)
	denom := d00*d11 - d01*d01
	if denom > 1e-12 {
		v, w := (d11*d20-d01*d21)/denom, (d00*d21-d01*d20)/denom
		if v >= 0 && w >= 0 && v+w <= 1 {
			q := rl.Vector3Add(a, rl.Vector3Add(rl.Vector3Scale(ab, v), rl.Vector3Scale(ac, w)))
			return rl.Vector3Distance(p, q)
		}
	}
	return min(rl.Vector3Distance(p, closestOnSegment(p, a, b)), rl.Vector3Distance(p, closestOnSegment(p, b, c)), rl.Vector3Distance(p, closestOnSegment(p, c, a)))
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
			out = append(out, fitPieces(bind[from].Translation, bind[to].Translation, g, c.pieces, from, to)...)
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

// fitPieces cuts the segment a-b, carried by bones from and to, into n
// capsules end to end, each fitted to the points alongside it. All but the
// end at b move with from: they're along its bone.
func fitPieces(a, b rl.Vector3, points []rl.Vector3, n, from, to int) []render.Capsule {
	n = max(n, 1)
	ab := rl.Vector3Subtract(b, a)
	l := rl.Vector3DotProduct(ab, ab)
	pieces := make([][]rl.Vector3, n)
	for _, p := range points {
		i := 0
		if l > 1e-12 {
			t := rl.Vector3DotProduct(rl.Vector3Subtract(p, a), ab) / l
			i = max(0, min(n-1, int(t*float32(n))))
		}
		pieces[i] = append(pieces[i], p)
	}
	var out []render.Capsule
	for i, pts := range pieces {
		start := rl.Vector3Lerp(a, b, float32(i)/float32(n))
		end := rl.Vector3Lerp(a, b, float32(i+1)/float32(n))
		k, ok := fitCapsule(start, end, pts)
		if !ok {
			continue
		}
		k.BoneA, k.BoneB = from, from
		if i == n-1 {
			k.BoneB = to
		}
		out = append(out, k)
	}
	return out
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
