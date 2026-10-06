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
		{"Charter Row", -4, -36},
		{"outside the gate", 0, 60},
		{"the arrival", arrival.X, arrival.Z},
		{"the Pads", padsAt.X, padsAt.Y},
		{"the caravan road", 3900, -120},
		{"the Fringe track", -850, 5400},
		{"the hold", holdAt.X, holdAt.Y},
		{"the farms", -1625, 12023},
		{"the salvage fields", -1375, 12020},
		{"the wind farm", -1600, 11900},
		{"the haven", havenAt.X, havenAt.Y},
		{"the wash out of the haven", -6000, -6200},
	} {
		if h := groundHeight(tt.x, tt.z); h != 0 {
			t.Errorf("%s: ground at %v, want level (0)", tt.name, h)
		}
	}
}

func TestTerrainRisesToTheEdge(t *testing.T) {
	var high float32
	for _, p := range []rl.Vector2{{X: 16000, Y: 0}, {X: -16000, Y: 3000}, {X: 2000, Y: -16000}, {X: -6000, Y: 16000}} {
		h := groundHeight(p.X, p.Y)
		high = max(high, h)
		if h < 80 {
			t.Errorf("at the world's edge %v, ground %v, want mountains", p, h)
		}
	}
	if high > 200 {
		t.Errorf("mountains %v m high, want under 200", high)
	}
}

func TestSeatsAreFarApart(t *testing.T) {
	seats := map[string]rl.Vector2{"Landfall": {}, "the Pads": padsAt, "the hold": holdAt, "the haven": havenAt}
	for a, pa := range seats {
		for b, pb := range seats {
			if a < b {
				if d := rl.Vector2Distance(pa, pb); d < 8000 {
					t.Errorf("%s and %s are %.0f m apart, want 8 km or more", a, b, d)
				}
			}
		}
	}
	for name, p := range map[string]rl.Vector2{"the Pads": padsAt, "the hold": holdAt, "the haven": havenAt} {
		if d := rl.Vector2Length(p); d > 15000 {
			t.Errorf("%s is %.0f m from Landfall, want 15 km at most", name, d)
		}
	}
}

func TestRocksAreOffTheRoads(t *testing.T) {
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, p := range placed {
		switch p.Piece {
		case "rock_small", "rock_large", "rock_spire", "dust_mound", "dry_brush", "dead_tree", "quiver_tree":
			if d, _ := roadDistance(p.At[0], p.At[2]); d < 9 {
				t.Errorf("%s at %v is %.1f m off a road: tools/world/landfall.py's ROADS and terrain.go's roads differ?", p.Piece, p.At, d)
			}
		}
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
	// On the caravan road, east of Landfall, walking north off it into
	// the dunes.
	l := newLandfall(t, rl.Vector3{X: 1000, Z: 167})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	l.tick(12 * 60)
	f := l.feet()
	if want := groundHeight(f.X, f.Z) + groundLevel; !l.cc.Grounded || f.Y < want-.3 || f.Y > want+.3 {
		t.Fatalf("out on the dunes at %v (grounded %v), want on the ground at %v", f, l.cc.Grounded, want)
	}
	if f.Z > 152 {
		t.Fatalf("walking north off the road, only got to z %v", f.Z)
	}
}
