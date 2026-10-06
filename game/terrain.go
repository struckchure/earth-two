package game

import (
	"math"
	"runtime"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// The terrain: the Red, worldSize across, centred on Landfall, with the
// other seats 8 to 15 km out across it (docs/settlement.md). It's level
// under each seat and along the roads between them, and rolls into dunes and
// ridges everywhere else, into canyons round the Quiet Book's haven, and up
// into mountains at the world's edge. groundHeight says how high it is
// anywhere; everything else (what's drawn, what's walked on, where the
// layout's pieces stand) comes from that.
//
// It's drawn in two layers. Round the player, detail chunks (chunkSize
// across, a vertex every chunkSize/chunkCells) follow them about; they
// collide, near the player. Everywhere, tiles (tileSize across, coarser)
// show the land to the horizon, sunk out of sight where the chunks are.
// Each is painted: the soil, the roads, the rock.
const (
	worldSize             = 32768
	chunkSize, chunkCells = 256, 64
	tileSize, tileCells   = 2048, 64
	detailRadius          = 3 // chunks each way round the player drawn in detail
	colliderRadius        = 1 // chunks each way round the player that collide
	chunkTexels           = 128
	tileTexels            = 64
	// tileSink is how far the tiles drop where chunks cover them.
	tileSink = 8
	// chunksPerFrame is how many chunks are rebuilt a frame when the
	// player moves on, so it doesn't stall.
	chunksPerFrame = 2
	// groundLevel is how far below 0 the ground's surface is: the deck's
	// floors stand on it, their tops at 0.
	groundLevel = -.05
	// underFloors is how much lower the drawn ground is under floors, so
	// the two can't flicker through each other (the collider isn't).
	underFloors = .5
)

// The seats, in the game's frame (X, and Z for Blender's -Y): where each
// one's middle is. tools/world/landfall.py has the same (PADS_AT etc.).
var (
	padsAt  = rl.Vector2{X: 9800, Y: -1800}
	holdAt  = rl.Vector2{X: -1500, Y: 12000}
	havenAt = rl.Vector2{X: -6500, Y: -6500}
)

// level is where the terrain is flat, on the XZ plane (X across, Y for
// the game's Z), from tools/world/landfall.py: under the seats.
var level = []rl.Rectangle{
	{X: -64, Y: -66, Width: 108, Height: 120},  // Landfall's dome, and a margin round it
	{X: -7, Y: 44, Width: 14, Height: 52},      // the road south from its gate
	{X: 9764, Y: -1850, Width: 60, Height: 90}, // the Pads
	// The Fringers' hold: its farms, salvage fields, camp, wind farm and road.
	{X: -1652, Y: 12004, Width: 54, Height: 40},
	{X: -1402, Y: 11998, Width: 56, Height: 54},
	{X: -1476, Y: 12000, Width: 32, Height: 30},
	{X: -1607, Y: 11842, Width: 14, Height: 146},
	{X: -1656, Y: 11993, Width: 312, Height: 14},
	{X: -6528, Y: -6528, Width: 56, Height: 56}, // the haven's pocket
}

// A road: its points, in the game's frame, how far either side of its
// middle it's level, and whether it's painted (the wash to the haven isn't
// a road, only a way through). tools/world/landfall.py has the same
// (ROADS): keep the two the same.
type road struct {
	name   string
	points []rl.Vector2
	half   float32
	paint  bool
}

var roads = []road{
	{"caravan road", []rl.Vector2{{X: 0, Y: 44}, {X: 0, Y: 90}, {X: 2200, Y: 260}, {X: 5600, Y: -500}, {X: 8300, Y: -1500}, {X: 9772, Y: -1772}}, 5, true},
	{"Fringe track", []rl.Vector2{{X: 0, Y: 90}, {X: -400, Y: 3200}, {X: -1300, Y: 7600}, {X: -1300, Y: 11000}, {X: -1500, Y: 12000}}, 3, true},
	{"hold road", []rl.Vector2{{X: -1650, Y: 12000}, {X: -1350, Y: 12000}}, 4, true},
	{"haven wash", []rl.Vector2{{X: -6500, Y: -6500}, {X: -6000, Y: -6200}, {X: -5400, Y: -5600}}, 4, false},
}

// roadDistance is how far (x, z) is from the nearest road, past its half
// width (0 or less on it), and whether that road is painted.
func roadDistance(x, z float32) (float32, bool) {
	best, painted := float32(math.Inf(1)), false
	for _, r := range roads {
		for i := 1; i < len(r.points); i++ {
			d := segmentDistance(r.points[i-1], r.points[i], x, z) - r.half
			if d < best {
				best, painted = d, r.paint
			}
		}
	}
	return best, painted
}

// segmentDistance is how far (x, z) is from the segment a to b.
func segmentDistance(a, b rl.Vector2, x, z float32) float32 {
	dx, dz := b.X-a.X, b.Y-a.Y
	t := clamp01(((x-a.X)*dx + (z-a.Y)*dz) / (dx*dx + dz*dz))
	return float32(math.Hypot(float64(x-a.X-t*dx), float64(z-a.Y-t*dz)))
}

// levelDistance is how far (x, z) is from the nearest level ground: 0 on
// it.
func levelDistance(x, z float32) float32 {
	d := float32(math.Inf(1))
	for _, r := range level {
		d = min(d, rectDistance(r, x, z))
	}
	if rd, _ := roadDistance(x, z); rd < d {
		d = max(0, rd)
	}
	return d
}

// groundHeight is how high the ground is at (x, z), above groundLevel.
func groundHeight(x, z float32) float32 {
	d := levelDistance(x, z)
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
	return full * max(-2, relief(x, z))
}

// relief is the land before it's levelled: dunes, long dunes, ridges,
// canyons round the haven, and mountains at the world's edge.
func relief(x, z float32) float32 {
	dunes := (fbm(x/45, z/45) - .5) * 9
	long := (fbm(x/420+11, z/260-4) - .5) * 26
	ridges := 3.5 * float32(math.Pow(float64(1-abs(2*fbm(x/110+7.3, z/160+1.9)-1)), 2))
	canyons := canyon(x, z) * 40 * float32(math.Pow(float64(1-abs(2*fbm(x/170-2.2, z/170+8.1)-1)), 1.5))
	rim := 140 * smoothstep(13500, 16200, max(abs(x), abs(z)))
	return dunes + long + ridges + canyons + rim
}

// canyon is how much (x, z) is in the canyon country round the haven:
// fully within 900 m of it, not at all past 1.7 km.
func canyon(x, z float32) float32 {
	return smoothstep(1700, 900, float32(math.Hypot(float64(x-havenAt.X), float64(z-havenAt.Y))))
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

// walkHeight is how high what's walked on is at (x, z), above groundLevel:
// the ground, but up at the floors' tops (0) where floors cover it. The
// floors have no colliders of their own, and without this feet sink
// through them to the ground under.
func walkHeight(x, z float32) float32 {
	h := groundHeight(x, z)
	for _, f := range floored {
		if x >= f.X && x <= f.X+f.Width && z >= f.Y && z <= f.Y+f.Height {
			return h - groundLevel
		}
	}
	return h
}

// groundColour is the ground's paint at (x, z): red soil, lighter and
// darker in drifts; packed dirt on the roads; darker rock in the canyons.
func groundColour(x, z float32) rl.Color {
	soil := mixColour(rl.NewColor(132, 66, 46, 255), rl.NewColor(178, 98, 64, 255), fbm(x/34, z/34))
	soil = mixColour(soil, rl.NewColor(196, 120, 80, 255), .35*smoothstep(.55, .8, fbm(x/9+5, z/90)))
	if c := canyon(x, z); c > 0 {
		soil = mixColour(soil, rl.NewColor(108, 58, 44, 255), c*smoothstep(.45, .7, fbm(x/60-9, z/60+3)))
	}
	if d, painted := roadDistance(x, z); painted && d < 1.5 {
		return mixColour(soil, rl.NewColor(118, 94, 80, 255), .85*smoothstep(1.5, -1, d))
	}
	return soil
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

// grid is a square of terrain size across, cells a side, round (cx, cz):
// its vertices (from height) and triangles, row by row along X then down Z,
// facing up.
func grid(cx, cz, size float32, cells int, height func(x, z float32) float32) ([]rl.Vector3, []uint32) {
	n := cells + 1
	step := size / float32(cells)
	vertices := make([]rl.Vector3, 0, n*n)
	for j := range n {
		for i := range n {
			x, z := cx-size/2+float32(i)*step, cz-size/2+float32(j)*step
			vertices = append(vertices, rl.Vector3{X: x, Y: groundLevel + height(x, z), Z: z})
		}
	}
	indices := make([]uint32, 0, cells*cells*6)
	for j := range cells {
		for i := range cells {
			a, b := uint32(j*n+i), uint32((j+1)*n+i)
			indices = append(indices, a, b, a+1, a+1, b, b+1)
		}
	}
	return vertices, indices
}

// chunkCollider is what's walked on in the chunk at (ci, cj): the ground
// as walkHeight has it, in the chunk's own frame (its entity sits at its
// middle).
func chunkCollider(ci, cj int) physics.Collider {
	c := chunkCentre(ci, cj)
	v, idx := grid(c.X, c.Y, chunkSize, chunkCells, walkHeight)
	for i := range v {
		v[i].X -= c.X
		v[i].Z -= c.Y
	}
	return physics.TriMesh(v, idx)
}

// chunkCentre is the middle of chunk (ci, cj); chunkOf is the chunk (x, z)
// is in.
func chunkCentre(ci, cj int) rl.Vector2 {
	return rl.Vector2{X: (float32(ci) + .5) * chunkSize, Y: (float32(cj) + .5) * chunkSize}
}

func chunkOf(x, z float32) (int, int) {
	return int(math.Floor(float64(x / chunkSize))), int(math.Floor(float64(z / chunkSize)))
}

// terrainNormal is the ground's normal at (x, z), from height, e apart.
func terrainNormal(height func(x, z float32) float32, x, z, e float32) rl.Vector3 {
	dx := height(x+e, z) - height(x-e, z)
	dz := height(x, z+e) - height(x, z-e)
	return rl.Vector3Normalize(rl.Vector3{X: -dx, Y: 2 * e, Z: -dz})
}

// shape sets mesh m (raylib's plane, size across, centred on its own
// origin) to the ground round (cx, cz) from height, sunk by sink, and
// uploads it.
func shape(m rl.Mesh, cx, cz, size float32, cells int, height func(x, z float32) float32, sink func(x, z float32) float32) {
	count := int(m.VertexCount)
	vertices := unsafe.Slice(m.Vertices, 3*count)
	normals := unsafe.Slice(m.Normals, 3*count)
	step := size / float32(cells)
	for i := range count {
		x, z := cx+vertices[3*i], cz+vertices[3*i+2]
		vertices[3*i+1] = groundLevel + height(x, z) - sink(x, z)
		n := terrainNormal(height, x, z, step)
		normals[3*i], normals[3*i+1], normals[3*i+2] = n.X, n.Y, n.Z
	}
	rl.UpdateMeshBuffer(m, 0, floatBytes(vertices), 0)
	rl.UpdateMeshBuffer(m, 2, floatBytes(normals), 0)
}

func floatBytes(f []float32) []byte {
	return unsafe.Slice((*byte)(unsafe.Pointer(unsafe.SliceData(f))), len(f)*4)
}

// paint is a texture of the ground round (cx, cz), size across, texels a
// side.
func paint(cx, cz, size float32, texels int) render.Texture {
	pixels := make([]byte, texels*texels*4)
	for y := range texels {
		for x := range texels {
			// raylib's plane runs its texture along X and down Z.
			wx := cx - size/2 + (float32(x)+.5)*size/float32(texels)
			wz := cz - size/2 + (float32(y)+.5)*size/float32(texels)
			c := groundColour(wx, wz)
			copy(pixels[(y*texels+x)*4:], []byte{c.R, c.G, c.B, 255})
		}
	}
	img := rl.NewImage(pixels, int32(texels), int32(texels), 1, rl.UncompressedR8g8b8a8)
	tex := rl.LoadTextureFromImage(img)
	runtime.KeepAlive(pixels) // the image is ours, not raylib's: it's never unloaded
	rl.SetTextureFilter(tex, rl.FilterBilinear)
	return render.Texture{Texture2D: tex}
}

// The terrain's entities: a detail chunk (its slot), a collider (its
// slot), a tile.
type (
	terrainChunk struct{ slot int }
	terrainBody  struct{ slot int }
	terrainTile  struct{}
)

// terrain is a resource: where the detail is, and what each of its slots
// holds.
type terrain struct {
	ci, cj   int // the chunk the detail is round
	chunks   []chunkSlot
	bodies   []bodySlot
	tiles    []tileSlot
	pending  []int // chunk slots waiting to be rebuilt
	textures *asset.Assets[render.Texture]
	mats     *asset.Assets[render.StandardMaterial]
}

type chunkSlot struct {
	ci, cj int // the chunk it's for
	bi, bj int // the chunk it shows, until it's rebuilt
	mesh   rl.Mesh
	mat    asset.Handle[render.StandardMaterial]
	tex    asset.Handle[render.Texture]
}

type bodySlot struct{ ci, cj int }

type tileSlot struct {
	cx, cz float32
	mesh   rl.Mesh
}

// detailWindow is the square the detail covers round chunk (ci, cj), on the XZ
// plane.
func detailWindow(ci, cj int) rl.Rectangle {
	w := float32(2*detailRadius+1) * chunkSize
	return rl.Rectangle{X: float32(ci-detailRadius) * chunkSize, Y: float32(cj-detailRadius) * chunkSize, Width: w, Height: w}
}

// sinkUnder sinks what's inside win by tileSink.
func sinkUnder(win rl.Rectangle) func(x, z float32) float32 {
	return func(x, z float32) float32 {
		if x > win.X && x < win.X+win.Width && z > win.Y && z < win.Y+win.Height {
			return tileSink
		}
		return 0
	}
}

func noSink(x, z float32) float32 { return 0 }

// spawnTerrain adds the terrain round at: the tiles everywhere, the detail
// and its colliders round at. The ground takes shadows but casts none: its
// gentle slopes would only shade themselves, speckled.
func spawnTerrain(cmd *illusion.Commands, meshes *asset.Assets[render.Mesh], mats *asset.Assets[render.StandardMaterial], textures *asset.Assets[render.Texture], at rl.Vector3) *terrain {
	t := &terrain{textures: textures, mats: mats}
	t.ci, t.cj = chunkOf(at.X, at.Z)
	sink := sinkUnder(detailWindow(t.ci, t.cj))
	h := float32(worldSize) / 2
	for tz := -h + tileSize/2; tz < h; tz += tileSize {
		for tx := -h + tileSize/2; tx < h; tx += tileSize {
			m := rl.GenMeshPlane(tileSize, tileSize, tileCells, tileCells)
			shape(m, tx, tz, tileSize, tileCells, drawnHeight, sink)
			mat := mats.Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.Add(paint(tx, tz, tileSize, tileTexels))})
			cmd.Spawn(
				illusion.C(render.Mesh3d{Mesh: meshes.Add(render.Mesh{Mesh: m})}),
				illusion.C(render.MeshMaterial3d{Material: mat}),
				illusion.C(transform.FromXYZ(tx, 0, tz)),
				illusion.C(terrainTile{}),
				illusion.C(render.NotShadowCaster{}),
			)
			t.tiles = append(t.tiles, tileSlot{cx: tx, cz: tz, mesh: m})
		}
	}
	slot := 0
	for dj := -detailRadius; dj <= detailRadius; dj++ {
		for di := -detailRadius; di <= detailRadius; di++ {
			ci, cj := t.ci+di, t.cj+dj
			c := chunkCentre(ci, cj)
			m := rl.GenMeshPlane(chunkSize, chunkSize, chunkCells, chunkCells)
			shape(m, c.X, c.Y, chunkSize, chunkCells, drawnHeight, noSink)
			tex := textures.Add(paint(c.X, c.Y, chunkSize, chunkTexels))
			mat := mats.Add(render.StandardMaterial{BaseColor: rl.White, Texture: tex})
			cmd.Spawn(
				illusion.C(render.Mesh3d{Mesh: meshes.Add(render.Mesh{Mesh: m})}),
				illusion.C(render.MeshMaterial3d{Material: mat}),
				illusion.C(transform.FromXYZ(c.X, 0, c.Y)),
				illusion.C(terrainChunk{slot}),
				illusion.C(render.NotShadowCaster{}),
			)
			t.chunks = append(t.chunks, chunkSlot{ci: ci, cj: cj, bi: ci, bj: cj, mesh: m, mat: mat, tex: tex})
			slot++
		}
	}
	for s := range (2*colliderRadius + 1) * (2*colliderRadius + 1) {
		b := t.bodyAt(s)
		c := chunkCentre(b.ci, b.cj)
		cmd.Spawn(illusion.C(transform.FromXYZ(c.X, 0, c.Y)), illusion.C(physics.Static), illusion.C(chunkCollider(b.ci, b.cj)), illusion.C(terrainBody{s}))
		t.bodies = append(t.bodies, b)
	}
	return t
}

// streamTerrain keeps the detail and the colliders round the player as
// they move: when they cross into another chunk, the chunks that fall out
// of the window are reused for the ones coming into it (a few a frame,
// nearest first), the colliders round them likewise (at once: they're
// what's stood on), and the tiles sink where the detail now is.
func streamTerrain(
	cmd *illusion.Commands,
	tr *illusion.Res[terrain],
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	chunks *illusion.Query2[transform.Transform, terrainChunk],
	bodies *illusion.Query2[transform.Transform, terrainBody],
) {
	t, ok := tr.TryGet()
	if !ok {
		return
	}
	_, player, ok := players.Single()
	if !ok {
		return
	}
	if ci, cj := chunkOf(player.Translation.X, player.Translation.Z); ci != t.ci || cj != t.cj {
		old := detailWindow(t.ci, t.cj)
		t.ci, t.cj = ci, cj
		t.retarget()
		t.resink(old)
		bodies.Each(func(e ecs.Entity, b *transform.Transform, s *terrainBody) {
			want := t.bodyAt(s.slot)
			if t.bodies[s.slot] == want {
				return
			}
			t.bodies[s.slot] = want
			c := chunkCentre(want.ci, want.cj)
			b.Translation = rl.Vector3{X: c.X, Z: c.Y}
			cmd.Entity(e).Insert(illusion.C(chunkCollider(want.ci, want.cj)))
		})
	}
	if len(t.pending) == 0 {
		return
	}
	n := min(chunksPerFrame, len(t.pending))
	rebuild := map[int]bool{}
	for _, s := range t.pending[:n] {
		rebuild[s] = true
	}
	t.pending = t.pending[n:]
	chunks.Each(func(_ ecs.Entity, c *transform.Transform, s *terrainChunk) {
		if !rebuild[s.slot] {
			return
		}
		slot := &t.chunks[s.slot]
		slot.bi, slot.bj = slot.ci, slot.cj
		at := chunkCentre(slot.ci, slot.cj)
		shape(slot.mesh, at.X, at.Y, chunkSize, chunkCells, drawnHeight, noSink)
		old := slot.tex
		slot.tex = t.textures.Add(paint(at.X, at.Y, chunkSize, chunkTexels))
		if m := t.mats.Get(slot.mat); m != nil {
			m.Texture = slot.tex
		}
		t.textures.Remove(old)
		c.Translation = rl.Vector3{X: at.X, Z: at.Y}
	})
}

// bodyAt is the chunk collider slot s should be on, round the detail's
// middle.
func (t *terrain) bodyAt(s int) bodySlot {
	side := 2*colliderRadius + 1
	return bodySlot{t.ci - colliderRadius + s%side, t.cj - colliderRadius + s/side}
}

// retarget hands the chunks the window has left to the ones it's taken
// on, and queues every slot that doesn't show its chunk yet (some may be
// left over from an earlier move) to be rebuilt, nearest the player first.
func (t *terrain) retarget() {
	wanted := map[[2]int]bool{}
	for dj := -detailRadius; dj <= detailRadius; dj++ {
		for di := -detailRadius; di <= detailRadius; di++ {
			wanted[[2]int{t.ci + di, t.cj + dj}] = true
		}
	}
	var free []int
	for s, c := range t.chunks {
		if wanted[[2]int{c.ci, c.cj}] {
			delete(wanted, [2]int{c.ci, c.cj})
		} else {
			free = append(free, s)
		}
	}
	i := 0
	for k := range wanted {
		t.chunks[free[i]].ci, t.chunks[free[i]].cj = k[0], k[1]
		i++
	}
	t.pending = t.pending[:0]
	for s, c := range t.chunks {
		if c.ci != c.bi || c.cj != c.bj {
			t.pending = append(t.pending, s)
		}
	}
	away := func(s int) int { return max(absInt(t.chunks[s].ci-t.ci), absInt(t.chunks[s].cj-t.cj)) }
	for i := 1; i < len(t.pending); i++ {
		for j := i; j > 0 && away(t.pending[j]) < away(t.pending[j-1]); j-- {
			t.pending[j], t.pending[j-1] = t.pending[j-1], t.pending[j]
		}
	}
}

func absInt(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

// resink sinks the tiles under where the detail is now, and raises them
// where it was (old).
func (t *terrain) resink(old rl.Rectangle) {
	win := detailWindow(t.ci, t.cj)
	overlaps := func(r rl.Rectangle, cx, cz float32) bool {
		return r.X < cx+tileSize/2 && r.X+r.Width > cx-tileSize/2 && r.Y < cz+tileSize/2 && r.Y+r.Height > cz-tileSize/2
	}
	for _, tile := range t.tiles {
		if overlaps(win, tile.cx, tile.cz) || overlaps(old, tile.cx, tile.cz) {
			shape(tile.mesh, tile.cx, tile.cz, tileSize, tileCells, drawnHeight, sinkUnder(win))
		}
	}
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

// terrainAround spawns the ground's colliders round (x, z), radius chunks
// each way, standing still: for a world without the streaming (tests).
func terrainAround(cmd *illusion.Commands, x, z float32, radius int) {
	ci, cj := chunkOf(x, z)
	for dj := -radius; dj <= radius; dj++ {
		for di := -radius; di <= radius; di++ {
			c := chunkCentre(ci+di, cj+dj)
			cmd.Spawn(illusion.C(transform.FromXYZ(c.X, 0, c.Y)), illusion.C(physics.Static), illusion.C(chunkCollider(ci+di, cj+dj)))
		}
	}
}
