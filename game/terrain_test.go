package game

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
)

func TestTerrainIsLevelWhereItMatters(t *testing.T) {
	for _, tt := range []struct {
		name string
		x, z float32
	}{
		{"the Hull", -16, 4},
		{"the arrival", arrival.X, arrival.Z},
		{"Charter Row", -4, -36},
		{"outside the gate", 0, 60},
		{"the caravan road", 80, 90},
		{"the farms", -125, 113},
		{"the salvage fields", 125, 110},
		{"the Fringer camp", 40, 104},
		{"the wind farm", -100, 0},
	} {
		if h := groundHeight(tt.x, tt.z); h != 0 {
			t.Errorf("%s: ground at %v, want level (0)", tt.name, h)
		}
	}
}

func TestTerrainRisesToTheEdge(t *testing.T) {
	var high float32
	for _, p := range []rl.Vector2{{X: 170, Y: 0}, {X: -170, Y: 30}, {X: 20, Y: -170}, {X: -60, Y: 170}} {
		h := groundHeight(p.X, p.Y)
		high = max(high, h)
		if h < 10 {
			t.Errorf("at the edge %v, ground %v, want hills", p, h)
		}
	}
	if high > 40 {
		t.Errorf("hills %v m high, want under 40", high)
	}
}

func TestPiecesStandOnTheGround(t *testing.T) {
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	lifted := 0
	for _, p := range placed {
		h := standOn(k, p)
		if under := groundHeight(p.At[0], p.At[2]); h > under+1e-4 {
			t.Errorf("%s at %v stands at %v, above the ground under its middle (%v)", p.Piece, p.At, h, under)
		}
		if h != 0 {
			lifted++
		}
	}
	if lifted == 0 {
		t.Error("nothing out in the Fringe stands off the level")
	}
}

func TestWalkingOffTheRoadOntoTheDunes(t *testing.T) {
	// On the caravan road, west of the gate, walking north off it into
	// the Fringe.
	l := newLandfall(t, rl.Vector3{X: -40, Z: 90})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	l.tick(12 * 60)
	f := l.feet()
	if want := groundHeight(f.X, f.Z) + groundLevel; !l.cc.Grounded || f.Y < want-.3 || f.Y > want+.3 {
		t.Fatalf("out on the dunes at %v (grounded %v), want on the ground at %v", f, l.cc.Grounded, want)
	}
	if f.Z > 75 {
		t.Fatalf("walking north off the road, only got to z %v", f.Z)
	}
}
