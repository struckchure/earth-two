package game

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
)

// Every piece with use spots is a piece the world has: a renamed bench
// would otherwise just stop being sat on.
func TestUseSpotsArePieces(t *testing.T) {
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	for name := range useSpots {
		if _, ok := k.Pieces[name]; !ok {
			t.Errorf("use spots for %q, which isn't a piece", name)
		}
	}
}

// Landfall has somewhere to sit and something to work on.
func TestLandfallHasUses(t *testing.T) {
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	u := newUses(k, placed)
	count := map[useKind]int{}
	for _, spots := range u.cells {
		for _, s := range spots {
			count[s.kind]++
		}
	}
	if count[useSit] < 10 || count[useRepair] < 5 {
		t.Errorf("Landfall has %d seats and %d machines to work on", count[useSit], count[useRepair])
	}
}

// A spot is found from in front of it, turned with its piece, and not from
// too far or from another floor.
func TestUseNear(t *testing.T) {
	u := &uses{cells: map[[2]int][]placedSpot{}}
	add := func(s placedSpot) {
		c := cellOf(s.at.X, s.at.Z)
		u.cells[c] = append(u.cells[c], s)
	}
	// A stool turned a quarter (facing +X) at (10, 0, 10), and a machine
	// facing -Z (its front toward +Z) at the origin.
	add(placedSpot{"stool", useSit, rl.Vector3{X: 10, Z: 10}, math.Pi / 2})
	add(placedSpot{"pump_unit", useRepair, rl.Vector3{Z: 1}, math.Pi})

	if s, ok := u.near(rl.Vector3{X: 10.5, Z: 10}); !ok || s.piece != "stool" {
		t.Errorf("in front of the stool: %v %v", s.piece, ok)
	}
	if s, ok := u.near(rl.Vector3{Z: 1.3}); !ok || s.piece != "pump_unit" {
		t.Errorf("in front of the pump: %v %v", s.piece, ok)
	}
	if _, ok := u.near(rl.Vector3{X: 4, Z: 4}); ok {
		t.Errorf("found something 4 m from anything")
	}
	if _, ok := u.near(rl.Vector3{Y: 3.6, Z: 1.3}); ok {
		t.Errorf("found the pump from the deck above it")
	}
	// Getting up from the stool, they stand in front of it.
	if at := standUpAt(placedSpot{at: rl.Vector3{X: 10, Z: 10}, facing: math.Pi / 2}); math.Abs(float64(at.X-10.55)) > 1e-3 {
		t.Errorf("stood up at %v, want 0.55 m out in front", at)
	}
}
