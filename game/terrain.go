package game

import (
	"math"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// The terrain: the red ground Landfall stands on and the Fringe round it,
// a square of side groundSize in cells of terrainCell. Inside the dome,
// along the roads and under the farms, the salvage fields, the camp and the
// wind farm it's level, at 0; away from them it rolls into dunes and
// ridges, and rises to hills round the edge, so the world doesn't just
// stop. It's made here, not in Blender: the collider, the drawn ground and
// where the layout's pieces stand all come from groundHeight.
const (
	terrainCell = 2
	// groundLevel is how far below 0 the ground's surface is: the deck's
	// floors stand on it, their tops at 0.
	groundLevel = -.05
	// underFloors is how much lower the drawn ground is under floors, so
	// the two can't flicker through each other (the collider isn't).
	underFloors = .5
)

// level is a part of the world the terrain is flat under, on the XZ plane
// (X across, Y for the game's Z), from tools/world/landfall.py.
var level = []rl.Rectangle{
	{X: -58, Y: -66, Width: 132, Height: 120}, // the dome, and a margin round it
	{X: -7, Y: 44, Width: 14, Height: 52},     // the road south from the gate
	{X: -156, Y: 83, Width: 312, Height: 14},  // the caravan road
	{X: -152, Y: 94, Width: 54, Height: 40},   // the farms
	{X: 98, Y: 88, Width: 56, Height: 54},     // the salvage fields
	{X: 24, Y: 90, Width: 32, Height: 30},     // the Fringer camp
	{X: -107, Y: -68, Width: 14, Height: 146}, // the wind farm
}

// groundHeight is how high the ground is at (x, z), above groundLevel.
func groundHeight(x, z float32) float32 {
	d := float32(math.Inf(1))
	for _, r := range level {
		d = min(d, rectDistance(r, x, z))
	}
	if d == 0 {
		return 0
	}
	// Level near the places above, the full terrain from 30 m or so off
	// them: their edges wander, so the level ground doesn't end in straight
	// embankments.
	d += (fbm(x/28+3.1, z/28-5.7) - .5) * 18
	full := smoothstep(3, 30, d)
	if full == 0 {
		return 0
	}
	// Dunes, long ridges across the wind, and hills round the edge.
	dunes := (fbm(x/45, z/45) - .5) * 9
	ridges := 3.5 * float32(math.Pow(float64(1-abs(2*fbm(x/110+7.3, z/160+1.9)-1)), 2))
	edge := max(abs(x), abs(z))
	rim := 22 * smoothstep(110, 176, edge)
	return full * max(-1.5, dunes+ridges+rim)
}

// drawnHeight is how high the ground is drawn at (x, z): groundHeight, but
// lower under the floors that cover it.
func drawnHeight(x, z float32) float32 {
	h := groundHeight(x, z)
	for _, f := range floored {
		if x > f.X && x < f.X+f.Width && z > f.Y && z < f.Y+f.Height {
			return h - underFloors
		}
	}
	return h
}

// rectDistance is how far (x, z) is from r: 0 inside it.
func rectDistance(r rl.Rectangle, x, z float32) float32 {
	dx := max(r.X-x, 0, x-(r.X+r.Width))
	dz := max(r.Y-z, 0, z-(r.Y+r.Height))
	return float32(math.Hypot(float64(dx), float64(dz)))
}

func smoothstep(a, b, v float32) float32 {
	t := clamp01((v - a) / (b - a))
	return t * t * (3 - 2*t)
}

func clamp01(v float32) float32 { return max(0, min(1, v)) }

func abs(v float32) float32 { return float32(math.Abs(float64(v))) }

// fbm is fractal value noise at (x, z): four octaves, 0 to 1.
func fbm(x, z float32) float32 {
	var sum, amp, total float32 = 0, 1, 0
	for i := range 4 {
		sum += amp * valueNoise(x, z, uint32(i))
		total += amp
		x, z, amp = x*2.03, z*2.03, amp*.5
	}
	return sum / total
}

// valueNoise is smooth noise, 0 to 1, from a lattice of hashed values.
func valueNoise(x, z float32, seed uint32) float32 {
	x0, z0 := float32(math.Floor(float64(x))), float32(math.Floor(float64(z)))
	fx, fz := x-x0, z-z0
	sx, sz := fx*fx*(3-2*fx), fz*fz*(3-2*fz)
	at := func(i, j float32) float32 { return lattice(int32(i), int32(j), seed) }
	top := at(x0, z0) + (at(x0+1, z0)-at(x0, z0))*sx
	bottom := at(x0, z0+1) + (at(x0+1, z0+1)-at(x0, z0+1))*sx
	return top + (bottom-top)*sz
}

// lattice is a fixed pseudo-random value, 0 to 1, for a lattice point.
func lattice(i, j int32, seed uint32) float32 {
	h := uint32(i)*0x27d4eb2d ^ uint32(j)*0x165667b1 ^ seed*0x9e3779b9
	h ^= h >> 15
	h *= 0x85ebca6b
	h ^= h >> 13
	h *= 0xc2b2ae35
	h ^= h >> 16
	return float32(h&0xffffff) / 0xffffff
}

// terrainGrid is the terrain's vertices (from height) and triangles, row
// by row along X then down Z, facing up.
func terrainGrid(height func(x, z float32) float32) ([]rl.Vector3, []uint32) {
	n := groundSize/terrainCell + 1
	h := float32(groundSize) / 2
	vertices := make([]rl.Vector3, 0, n*n)
	for j := range n {
		for i := range n {
			x, z := -h+float32(i*terrainCell), -h+float32(j*terrainCell)
			vertices = append(vertices, rl.Vector3{X: x, Y: groundLevel + height(x, z), Z: z})
		}
	}
	indices := make([]uint32, 0, (n-1)*(n-1)*6)
	for j := range n - 1 {
		for i := range n - 1 {
			a, b := uint32(j*n+i), uint32((j+1)*n+i)
			indices = append(indices, a, b, a+1, a+1, b, b+1)
		}
	}
	return vertices, indices
}

// terrainCollider is the ground everyone walks on.
func terrainCollider() physics.Collider {
	return physics.TriMesh(terrainGrid(groundHeight))
}

// terrainNormal is the ground's normal at (x, z), from height.
func terrainNormal(height func(x, z float32) float32, x, z float32) rl.Vector3 {
	const e = terrainCell
	dx := height(x+e, z) - height(x-e, z)
	dz := height(x, z+e) - height(x, z-e)
	return rl.Vector3Normalize(rl.Vector3{X: -dx, Y: 2 * e, Z: -dz})
}

// terrainMesh is the drawn ground: raylib's plane of the right size and
// cells, its heights and normals set to drawnHeight's.
func terrainMesh() rl.Mesh {
	n := groundSize / terrainCell
	m := rl.GenMeshPlane(groundSize, groundSize, n, n)
	count := int(m.VertexCount)
	vertices := unsafe.Slice(m.Vertices, 3*count)
	normals := unsafe.Slice(m.Normals, 3*count)
	for i := range count {
		x, z := vertices[3*i], vertices[3*i+2]
		vertices[3*i+1] = drawnHeight(x, z)
		nv := terrainNormal(drawnHeight, x, z)
		normals[3*i], normals[3*i+1], normals[3*i+2] = nv.X, nv.Y, nv.Z
	}
	rl.UpdateMeshBuffer(m, 0, floatBytes(vertices), 0)
	rl.UpdateMeshBuffer(m, 2, floatBytes(normals), 0)
	return m
}

func floatBytes(f []float32) []byte {
	return unsafe.Slice((*byte)(unsafe.Pointer(unsafe.SliceData(f))), len(f)*4)
}

// spawnOnTerrain places the layout's pieces, each stood on the ground: at
// the lowest the ground is under it, so on a slope it sinks into the high
// side rather than floating off the low one. (On the level, in town, that's
// where the layout put it.)
func spawnOnTerrain(cmd *illusion.Commands, k *world.Kit, placed []world.Placement) error {
	for _, p := range placed {
		p.At[1] += standOn(k, p)
		if err := k.Place(cmd, p); err != nil {
			return err
		}
	}
	return nil
}

// standOn is how high the ground is under a placed piece: its lowest,
// under the middle and the corners of what it collides with.
func standOn(k *world.Kit, p world.Placement) float32 {
	at := rl.Vector3{X: p.At[0], Y: p.At[1], Z: p.At[2]}
	low := groundHeight(at.X, at.Z)
	piece, ok := k.Pieces[p.Piece]
	if !ok {
		return low
	}
	turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
	for _, c := range piece.Colliders {
		r, _ := footprint(c, at, turn)
		for _, x := range []float32{r.X, r.X + r.Width} {
			for _, z := range []float32{r.Y, r.Y + r.Height} {
				low = min(low, groundHeight(x, z))
			}
		}
	}
	return low
}
