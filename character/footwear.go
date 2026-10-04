package character

import (
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
)

// footwearCapsules fits the shoe uppers, carried by their own bone weights.
// Segmenting along the foot keeps the tongue and toe at their own heights;
// boot shafts weighted to the calf follow the shin independently of the foot.
func footwearCapsules(model rl.Model) []render.Capsule {
	byBone := map[int][]rl.Vector3{}
	for _, m := range model.GetMeshes() {
		if m.Vertices == nil || m.BoneIndices == nil || m.BoneWeights == nil {
			continue
		}
		vertices := unsafe.Slice((*rl.Vector3)(unsafe.Pointer(m.Vertices)), m.VertexCount)
		ids, weights := unsafe.Slice(m.BoneIndices, 4*int(m.VertexCount)), unsafe.Slice(m.BoneWeights, 4*int(m.VertexCount))
		for i, p := range vertices {
			best := 0
			for j := 1; j < 4; j++ {
				if weights[4*i+j] > weights[4*i+best] {
					best = j
				}
			}
			bone := int(ids[4*i+best])
			byBone[bone] = append(byBone[bone], p)
		}
	}
	var out []render.Capsule
	// Skeleton order makes the fit deterministic when capsules overlap.
	for bone := range model.Skeleton.GetBones() {
		out = append(out, fitShoeUpper(byBone[bone], bone)...)
	}
	return out
}

func fitShoeUpper(points []rl.Vector3, bone int) []render.Capsule {
	if len(points) == 0 {
		return nil
	}
	lo, hi := points[0].Z, points[0].Z
	for _, p := range points[1:] {
		lo, hi = min(lo, p.Z), max(hi, p.Z)
	}
	const pieces = 6
	bins := make([][]rl.Vector3, pieces)
	span := max(hi-lo, 1e-6)
	for _, p := range points {
		i := min(pieces-1, int((p.Z-lo)/span*pieces))
		bins[i] = append(bins[i], p)
	}
	var out []render.Capsule
	for _, pts := range bins {
		if len(pts) < 3 {
			continue
		}
		low, high := pts[0], pts[0]
		for _, p := range pts[1:] {
			low.X, low.Z = min(low.X, p.X), min(low.Z, p.Z)
			high.X, high.Y, high.Z = max(high.X, p.X), max(high.Y, p.Y), max(high.Z, p.Z)
		}
		r := max((high.X-low.X)/2, 0.008)
		centre := rl.Vector3{X: (low.X + high.X) / 2, Y: high.Y - r}
		a, b := centre, centre
		a.Z, b.Z = low.Z, high.Z
		out = append(out, render.Capsule{A: a, B: b, Radius: r, BoneA: bone, BoneB: bone})
	}
	return out
}

// cuffClearance loosens only vertices near shoe uppers. Shorts and bare feet
// keep their usual fit; cuffs get enough range to clear the selected footwear.
func cuffClearance(p rl.Vector3, shoes []render.Capsule) float32 {
	var freedom float32
	for _, shoe := range shoes {
		d := rl.Vector3Distance(p, closestOnSegment(p, shoe.A, shoe.B)) - shoe.Radius
		if d < 0.025 {
			// Allow the collision projection plus a little settling room.
			freedom = max(freedom, min(0.10, max(0.015, 0.025-d)))
		}
	}
	return freedom
}
