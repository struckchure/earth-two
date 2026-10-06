package game

import (
	"math"

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

// The Fringe's scatter: dry brush and stones, and now and then
// a boulder or a dead tree, wherever the player goes, so the open ground
// between the seats isn't bare. It's laid out cell by cell from a fixed
// seed, so a place always has the same things on it, and it follows the
// player: a window of cells round them, its slots handed on to the cells
// coming into it as they move, as the terrain's chunks are. Nothing is
// spawned or despawned once it's made; what isn't needed is parked out of
// sight under the world.
const (
	scatterCell   = 40 // metres across a cell
	scatterRadius = 3  // cells each way round the player's
	scatterPer    = 14 // the most things a cell holds
	scatterBodies = 2  // the most colliders a thing has
	// scatterClear is how far the scatter keeps from level ground: the
	// seats, and the roads between them.
	scatterClear = 8
	// scatterParked is how deep what isn't needed is put.
	scatterParked = -1000
)

// scatterKind is a piece the scatter uses: how often, out of the weights'
// sum, and, for those without colliders, how much it's scaled up or down.
type scatterKind struct {
	piece      string
	weight     float32
	minS, maxS float32
}

var scatterKinds = []scatterKind{
	{"dry_brush", 40, 1.3, 2.4},
	{"rock_small", 26, 1, 1},
	{"rock_large", 10, 1, 1},
	{"dead_tree", 4, 1, 1},
	{"quiver_tree", 4, 1, 1},
}

// brushClump is how many tufts of brush grow round one, and how far out.
const (
	brushClump  = 3
	brushSpread = 1.6
)

// scatterNoBodies is the ground kept free of anything to run into: round
// the Pads, where the vehicles are tried out over the dunes (see
// dunes_test.go). Brush and mounds still grow there.
var scatterNoBodies = []struct {
	at     rl.Vector2
	radius float32
}{{padsAt, 600}}

// scatterThing is one thing in a cell: which piece, where its origin
// stands, turned yaw about Up, and scaled.
type scatterThing struct {
	piece string
	at    rl.Vector3
	yaw   float32
	scale float32
}

// scatterIn is what's in cell (ci, cj): the same every time. Its heights
// are the ground's at each thing's middle; the stream stands them on it.
func scatterIn(ci, cj int) []scatterThing {
	seed := uint32(int32(ci))*0x9e3779b1 ^ uint32(int32(cj))*0x85ebca77
	next := func() float32 {
		seed ^= seed << 13
		seed ^= seed >> 17
		seed ^= seed << 5
		return float32(seed&0xffffff) / 0xffffff
	}
	for range 3 {
		next()
	}
	x0, z0 := float32(ci)*scatterCell, float32(cj)*scatterCell
	// Some stretches are barer than others.
	busy := fbm(x0/300+17, z0/300-5)
	n := 2 + int(6*smoothstep(.3, .7, busy))
	var total float32
	for _, k := range scatterKinds {
		total += k.weight
	}
	var out []scatterThing
	put := func(kind scatterKind, x, z, yaw, size float32) {
		if len(out) >= scatterPer || levelDistance(x, z) < scatterClear {
			return
		}
		if kind.minS == kind.maxS && noBodies(x, z) {
			return
		}
		out = append(out, scatterThing{
			piece: kind.piece,
			at:    rl.Vector3{X: x, Y: groundHeight(x, z), Z: z},
			yaw:   yaw,
			scale: kind.minS + (kind.maxS-kind.minS)*size,
		})
	}
	for range n {
		x, z := x0+next()*scatterCell, z0+next()*scatterCell
		pick, yaw, size := next()*total, next()*2*math.Pi, next()
		kind := scatterKinds[len(scatterKinds)-1]
		for _, k := range scatterKinds {
			if pick < k.weight {
				kind = k
				break
			}
			pick -= k.weight
		}
		put(kind, x, z, yaw, size)
		if kind.piece != "dry_brush" {
			continue
		}
		// Brush grows in clumps.
		for range brushClump {
			a, d := next()*2*math.Pi, next()*brushSpread
			put(kind, x+d*float32(math.Cos(float64(a))), z+d*float32(math.Sin(float64(a))), next()*2*math.Pi, next()*size)
		}
	}
	return out
}

// noBodies reports whether (x, z) is kept free of colliders.
func noBodies(x, z float32) bool {
	for _, a := range scatterNoBodies {
		if rl.Vector2Distance(a.at, rl.Vector2{X: x, Y: z}) < a.radius {
			return true
		}
	}
	return false
}

// scatter is a resource: the window's middle cell, and the cell each slot
// holds.
type scatter struct {
	ci, cj int
	slots  [][2]int
	made   bool
}

// scatterModel and scatterBody are a slot's things and their colliders.
type (
	scatterModel struct{ slot, item int }
	scatterBody  struct{ slot, item, n int }
)

func scatterCellOf(x, z float32) (int, int) {
	return int(math.Floor(float64(x / scatterCell))), int(math.Floor(float64(z / scatterCell)))
}

// scatterWindow is the cells round (ci, cj).
func scatterWindow(ci, cj int) [][2]int {
	var out [][2]int
	for dj := -scatterRadius; dj <= scatterRadius; dj++ {
		for di := -scatterRadius; di <= scatterRadius; di++ {
			out = append(out, [2]int{ci + di, cj + dj})
		}
	}
	return out
}

// streamScatter makes the scatter's slots round the player the first time,
// and after that, when they move into another cell, hands the slots the
// window's left to the cells it's taken on, and moves their things there.
func streamScatter(
	cmd *illusion.Commands,
	sc *illusion.Res[scatter],
	kit *illusion.Res[world.Kit],
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	models *illusion.Query3[transform.Transform, render.Model3d, scatterModel],
	bodies *illusion.Query2[transform.Transform, scatterBody],
) {
	s, k := sc.Get(), kit.Get()
	_, player, ok := players.Single()
	if !ok {
		return
	}
	ci, cj := scatterCellOf(player.Translation.X, player.Translation.Z)
	if !s.made {
		s.ci, s.cj, s.made = ci, cj, true
		s.slots = scatterWindow(ci, cj)
		for slot, cell := range s.slots {
			things := scatterIn(cell[0], cell[1])
			for item := range scatterPer {
				model, at, cols := scatterPose(k, things, item, cell)
				cmd.Spawn(
					illusion.C(render.Model3d{Model: model}),
					illusion.C(at),
					illusion.C(scatterModel{slot, item}),
				)
				for n := range scatterBodies {
					where, box := cols(n)
					cmd.Spawn(illusion.C(where), illusion.C(physics.Static), illusion.C(box), illusion.C(scatterBody{slot, item, n}))
				}
			}
		}
		return
	}
	if ci == s.ci && cj == s.cj {
		return
	}
	s.ci, s.cj = ci, cj
	want := map[[2]int]bool{}
	for _, c := range scatterWindow(ci, cj) {
		want[c] = true
	}
	held := map[[2]int]bool{}
	var free []int
	for slot, c := range s.slots {
		if want[c] {
			held[c] = true
		} else {
			free = append(free, slot)
		}
	}
	moved := map[int][]scatterThing{}
	for _, c := range scatterWindow(ci, cj) {
		if held[c] || len(free) == 0 {
			continue
		}
		slot := free[0]
		free = free[1:]
		s.slots[slot] = c
		moved[slot] = scatterIn(c[0], c[1])
	}
	if len(moved) == 0 {
		return
	}
	models.Each(func(_ ecs.Entity, tr *transform.Transform, m *render.Model3d, id *scatterModel) {
		things, ok := moved[id.slot]
		if !ok {
			return
		}
		model, at, _ := scatterPose(k, things, id.item, s.slots[id.slot])
		m.Model, *tr = model, at
	})
	bodies.Each(func(e ecs.Entity, tr *transform.Transform, id *scatterBody) {
		things, ok := moved[id.slot]
		if !ok {
			return
		}
		_, _, cols := scatterPose(k, things, id.item, s.slots[id.slot])
		where, box := cols(id.n)
		*tr = where
		cmd.Entity(e).Insert(illusion.C(box))
	})
}

// scatterPose is how a slot's item-th thing is placed, in a cell holding
// things: its model, its transform (stood on the ground, at its lowest
// under it), and its colliders by number, each a transform and a
// box. An item past the cell's things, and a collider past a thing's, are
// parked out of sight.
func scatterPose(k *world.Kit, things []scatterThing, item int, cell [2]int) (model asset.Handle[render.Model], at transform.Transform, cols func(n int) (transform.Transform, physics.Collider)) {
	parked := rl.Vector3{X: float32(cell[0]) * scatterCell, Y: scatterParked, Z: float32(cell[1]) * scatterCell}
	park := func(int) (transform.Transform, physics.Collider) {
		return transform.FromTranslation(parked), physics.Cuboid(.1, .1, .1)
	}
	if item >= len(things) {
		return k.Model(scatterKinds[0].piece), transform.FromTranslation(parked), park
	}
	t := things[item]
	piece := k.Pieces[t.piece]
	turn := rl.QuaternionFromAxisAngle(transform.Up, t.yaw)
	pos := t.at
	for _, c := range piece.Colliders {
		r, _ := footprint(c, pos, turn)
		for _, x := range []float32{r.X, r.X + r.Width} {
			for _, z := range []float32{r.Y, r.Y + r.Height} {
				pos.Y = min(pos.Y, groundHeight(x, z))
			}
		}
	}
	// A little into the ground, so no edge of it floats on a slope.
	pos.Y += groundLevel - .08*t.scale
	at = transform.FromTranslation(pos).WithRotation(turn).WithScale(t.scale)
	cols = func(n int) (transform.Transform, physics.Collider) {
		if n >= len(piece.Colliders) {
			return park(n)
		}
		c := piece.Colliders[n]
		center, rot := c.In(pos, turn)
		return transform.FromTranslation(center).WithRotation(rot), physics.Cuboid(c.Size[0], c.Size[1], c.Size[2])
	}
	return k.Model(t.piece), at, cols
}
