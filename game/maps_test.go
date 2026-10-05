package game

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
)

func TestMapKey(t *testing.T) {
	m := &menu{}
	w, o := menuWardrobe(), &character.Outfit{}
	m.navigate(nav{mapKey: true}, w, o)
	if m.screen() != mapping {
		t.Fatalf("M in play: %v, want the map", m.screen())
	}
	m.navigate(nav{down: true, enter: true}, w, o) // nothing to choose on it
	m.navigate(nav{mapKey: true}, w, o)
	if m.screen() != playing {
		t.Fatalf("M on the map: %v, want back to play", m.screen())
	}
	m.navigate(nav{mapKey: true}, w, o)
	m.navigate(nav{back: true}, w, o)
	if m.screen() != playing {
		t.Fatalf("Esc on the map: %v, want back to play", m.screen())
	}
	m.navigate(nav{back: true}, w, o)
	m.navigate(nav{mapKey: true}, w, o)
	if m.screen() != paused {
		t.Fatalf("M in the pause menu: %v, want it to stay paused", m.screen())
	}
}

func TestHeading(t *testing.T) {
	for _, tt := range []struct {
		forward rl.Vector3
		want    float32
	}{
		{rl.Vector3{Z: -1}, 0},  // north
		{rl.Vector3{X: 1}, 90},  // east
		{rl.Vector3{Z: 1}, 180}, // south
		{rl.Vector3{X: -1}, 270},
	} {
		if got := heading(tt.forward); got < tt.want-.01 || got > tt.want+.01 {
			t.Errorf("heading(%v) = %v, want %v", tt.forward, got, tt.want)
		}
	}
}

func TestClip(t *testing.T) {
	frame := rl.Rectangle{X: 0, Y: 0, Width: 100, Height: 100}
	if got := clip(rl.Rectangle{X: 90, Y: -10, Width: 20, Height: 30}, frame); got != (rl.Rectangle{X: 90, Y: 0, Width: 10, Height: 20}) {
		t.Errorf("clip over the corner = %v", got)
	}
	if got := clip(rl.Rectangle{X: 200, Y: 0, Width: 5, Height: 5}, frame); got.Width != 0 {
		t.Errorf("clip outside = %v, want empty", got)
	}
}

func TestLandfallMap(t *testing.T) {
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	m := newWorldMap(k, placed)
	colliders := 0
	for _, p := range placed {
		colliders += len(k.Pieces[p.Piece].Colliders)
	}
	// One for each collider, and the dome's inside and the two floors.
	if want := colliders + 3; len(m.marks) != want {
		t.Errorf("%d marks, want %d", len(m.marks), want)
	}
	in := func(r rl.Rectangle, p rl.Vector2) bool {
		return p.X >= r.X && p.X <= r.X+r.Width && p.Y >= r.Y && p.Y <= r.Y+r.Height
	}
	if !in(m.bounds, rl.Vector2{X: arrival.X, Y: arrival.Z}) {
		t.Errorf("the arrival %v is off the map %v", arrival, m.bounds)
	}
	// A dome wall on the south side, left of the gate, is on it.
	found := false
	for _, mk := range m.marks {
		if in(mk.r, rl.Vector2{X: -10, Y: 44}) {
			found = true
		}
	}
	if !found {
		t.Error("no mark where the dome's south wall is")
	}
}
