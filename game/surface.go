package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
)

// footing is what's underfoot, for how a step sounds and whether it raises
// dust.
type footing uint8

const (
	sand    footing = iota // the Red's dunes, and the banks against the dome
	soil                   // the dome's beds and lawns, and the packed dirt of the roads
	paving                 // concrete and the Hull's deck: Charter Row, the Pads
	grating                // kit underfoot: catwalks, stairs, deck slabs, plinths
	rug                    // a rug on the floor
	rock                   // the canyon country
)

// step is the sound of a step on s, and how it's pitched.
func (s footing) step() (string, float32) {
	switch s {
	case soil:
		return "step_soil", 1
	case paving:
		return "step_paving", 1
	case grating:
		return "step_grating", 1
	case rug:
		return "step_rug", 1
	case rock:
		return "step_paving", .85
	}
	return "step_sand", 1
}

// loose reports whether s is loose enough to kick up dust.
func (s footing) loose() bool { return s == sand || s == soil }

// pavingCell is the size of Charter Row's paving tiles, and the grid
// they're found on; rugReach how far from its middle a rug is underfoot.
const (
	pavingCell = 2
	rugReach   = 1.2
)

// soundscape is a resource: what the cues need to know about the world
// that the world doesn't say: where it's paved and where there are rugs,
// what's making a noise where, and where the bell is.
type soundscape struct {
	// paved is the paving tiles' middles, by the pavingCell they're in.
	paved map[[2]int][]rl.Vector2
	rugs  []rl.Vector3
	// sources are the places each loop is heard from, by the loop's name:
	// the machines' hums, the market's chatter, the fountain.
	sources map[string][]rl.Vector3
	bell    rl.Vector3
	hasBell bool
}

// sourceLoops are the pieces that make a noise, and the loop each makes.
var sourceLoops = map[string]string{
	"generator": "generator", "pump_unit": "generator",
	"air_scrubber": "fans", "air_fan": "fans", "hull_vent": "fans",
	"fountain":     "fountain",
	"market_stall": "market", "food_stall": "market", "parts_stall": "market",
}

func newSoundscape(k *world.Kit, placed []world.Placement) *soundscape {
	s := &soundscape{paved: map[[2]int][]rl.Vector2{}, sources: map[string][]rl.Vector3{}}
	for _, p := range placed {
		at := rl.Vector3{X: p.At[0], Y: p.At[1], Z: p.At[2]}
		switch p.Piece {
		case "charter_paving":
			c := pavingOf(at.X, at.Z)
			s.paved[c] = append(s.paved[c], rl.Vector2{X: at.X, Y: at.Z})
			continue
		case "rug", "rug_runner":
			s.rugs = append(s.rugs, at)
			continue
		case "floor_bell":
			at.Y += standOn(k, p)
			s.bell, s.hasBell = rl.Vector3Add(at, rl.Vector3{Y: 2}), true
		}
		if loop, ok := sourceLoops[p.Piece]; ok {
			at.Y += standOn(k, p) + 1
			s.sources[loop] = append(s.sources[loop], at)
		}
	}
	return s
}

func pavingOf(x, z float32) [2]int {
	return [2]int{int(math.Floor(float64(x / pavingCell))), int(math.Floor(float64(z / pavingCell)))}
}

// onPaving reports whether (x, z) is on a paving tile: within half a tile
// of one's middle, in its cell or one next to it.
func (s *soundscape) onPaving(x, z float32) bool {
	c := pavingOf(x, z)
	for dx := -1; dx <= 1; dx++ {
		for dz := -1; dz <= 1; dz++ {
			for _, m := range s.paved[[2]int{c[0] + dx, c[1] + dz}] {
				if abs(m.X-x) <= pavingCell/2 && abs(m.Y-z) <= pavingCell/2 {
					return true
				}
			}
		}
	}
	return false
}

// nearest is the source of loop nearest p, and how far it is.
func (s *soundscape) nearest(loop string, p rl.Vector3) (rl.Vector3, float32, bool) {
	best, far, found := rl.Vector3{}, float32(math.Inf(1)), false
	for _, at := range s.sources[loop] {
		if d := rl.Vector3Distance(at, p); d < far {
			best, far, found = at, d, true
		}
	}
	return best, far, found
}

// surfaceAt is what's underfoot at feet: onKit is whether what's under
// them is a piece of kit (a ray down hit one of the world's colliders, not
// the ground).
func (s *soundscape) surfaceAt(feet rl.Vector3, onKit bool) footing {
	x, z := feet.X, feet.Z
	for _, r := range s.rugs {
		if dx, dz := r.X-x, r.Z-z; dx*dx+dz*dz < rugReach*rugReach && abs(r.Y-feet.Y) < .5 {
			return rug
		}
	}
	if onKit {
		return grating
	}
	for _, f := range floored {
		if x >= f.X && x <= f.X+f.Width && z >= f.Y && z <= f.Y+f.Height {
			return paving
		}
	}
	if s.onPaving(x, z) {
		return paving
	}
	if d, _ := roadDistance(x, z); d <= 0 {
		return soil
	}
	if rectDistance(domeWalls, x, z) == 0 {
		return soil
	}
	if canyon(x, z) > .5 {
		return rock
	}
	return sand
}
