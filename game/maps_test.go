package game

import (
	"math"
	"slices"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion/render"
)

func loadMapForTest(t testing.TB) *worldMap {
	t.Helper()
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	return newWorldMap(k, placed)
}

// Reference the original draw selection, including its layering order.
func visibleMapMarks(f mapFrame, marks []mark) []mark {
	var out []mark
	reach := float32(math.Hypot(float64(f.screen.Width), float64(f.screen.Height))) / 2 / f.scale
	for _, mk := range marks {
		if f.turned() {
			half := rl.Vector2{X: mk.r.Width / 2, Y: mk.r.Height / 2}
			centre := rl.Vector2{X: mk.r.X + half.X, Y: mk.r.Y + half.Y}
			if rl.Vector2Distance(centre, f.at)-rl.Vector2Length(half) > reach {
				continue
			}
		} else {
			a := f.toScreen(rl.Vector2{X: mk.r.X, Y: mk.r.Y})
			r := clip(rl.Rectangle{X: a.X, Y: a.Y, Width: mk.r.Width * f.scale, Height: mk.r.Height * f.scale}, f.screen)
			if r.Width <= 0 || r.Height <= 0 {
				continue
			}
		}
		out = append(out, mk)
	}
	return out
}

func TestIndexedMapPreservesDrawSelection(t *testing.T) {
	m := loadMapForTest(t)
	for _, at := range []rl.Vector2{{}, padsAt, holdAt, havenAt, {X: -256, Y: 256}, {X: 5600, Y: -500}, {X: 16300, Y: -16300}} {
		for _, scale := range []float32{150. / minimapRange, .01, .5, 8} {
			for _, angle := range []float64{0, .3, math.Pi / 2, math.Pi} {
				f := mapFrame{screen: rl.Rectangle{X: 16, Y: 16, Width: 150, Height: 150}, at: at, scale: scale,
					up: rl.Vector2{X: float32(math.Sin(angle)), Y: -float32(math.Cos(angle))}}
				reach := float32(math.Hypot(150, 150)) / 2 / scale
				got := visibleMapMarks(f, m.marksNear(at, reach+1/scale))
				want := visibleMapMarks(f, m.marks)
				if !slices.Equal(got, want) {
					t.Fatalf("draw selection changed at %v, scale %v, angle %v: %d marks vs %d", at, scale, angle, len(got), len(want))
				}
			}
		}
	}
}

var benchmarkMapCount int

func BenchmarkMinimapSelection(b *testing.B) {
	m := loadMapForTest(b)
	for _, place := range []struct {
		name string
		at   rl.Vector2
	}{{"Pads", padsAt}, {"Landfall", rl.Vector2{}}, {"Fringe", rl.Vector2{X: 4200, Y: -300}}} {
		f := mapFrame{screen: rl.Rectangle{Width: 150, Height: 150}, at: place.at, scale: 150. / minimapRange, up: rl.Vector2{X: .6, Y: -.8}}
		reach := float32(math.Hypot(150, 150)) / 2 / f.scale
		b.Run(place.name+"/linear", func(b *testing.B) {
			for i := 0; i < b.N; i++ {
				benchmarkMapCount = len(visibleMapMarks(f, m.marks))
			}
		})
		b.Run(place.name+"/indexed", func(b *testing.B) {
			for i := 0; i < b.N; i++ {
				benchmarkMapCount = len(visibleMapMarks(f, m.marksNear(f.at, reach+1/f.scale)))
			}
		})
	}
}

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

func near(a, b rl.Vector2) bool { return rl.Vector2Distance(a, b) < 1e-3 }

// North up, the frame is the world as it was: X across, Z down.
func TestMapFrameNorthUp(t *testing.T) {
	f := mapFrame{screen: rl.Rectangle{Width: 100, Height: 100}, at: rl.Vector2{X: 10, Y: 20}, scale: 2}
	if got := f.toScreen(rl.Vector2{X: 15, Y: 18}); !near(got, rl.Vector2{X: 60, Y: 46}) {
		t.Errorf("toScreen = %v, want (60, 46)", got)
	}
	if f.turned() {
		t.Error("a frame with no up turns")
	}
}

// Turned to look east, what's east is up and what's north is to the left;
// a click finds the point it was drawn at.
func TestMapFrameTurns(t *testing.T) {
	f := mapFrame{screen: rl.Rectangle{Width: 100, Height: 100}, at: rl.Vector2{X: 10, Y: 20}, scale: 2, up: rl.Vector2{X: 1}}
	if got := f.toScreen(rl.Vector2{X: 20, Y: 20}); !near(got, rl.Vector2{X: 50, Y: 30}) {
		t.Errorf("10 m east = %v, want straight up (50, 30)", got)
	}
	if got := f.toScreen(rl.Vector2{X: 10, Y: 10}); !near(got, rl.Vector2{X: 30, Y: 50}) {
		t.Errorf("10 m north = %v, want to the left (30, 50)", got)
	}
	if got := f.screenDir(rl.Vector2{Y: 1}); !near(got, rl.Vector2{X: 1}) {
		t.Errorf("facing south points %v on the frame, want right", got)
	}
	p := rl.Vector2{X: 13, Y: 27}
	if got := f.toWorld(f.toScreen(p)); !near(got, p) {
		t.Errorf("toWorld(toScreen(%v)) = %v", p, got)
	}
}

func TestEdgePoint(t *testing.T) {
	r := rl.Rectangle{X: 0, Y: 0, Width: 100, Height: 50}
	for _, tt := range []struct{ dir, want rl.Vector2 }{
		{rl.Vector2{X: 1}, rl.Vector2{X: 95, Y: 25}},
		{rl.Vector2{Y: -1}, rl.Vector2{X: 50, Y: 5}},
		{rl.Vector2Normalize(rl.Vector2{X: 1, Y: 1}), rl.Vector2{X: 70, Y: 45}},
	} {
		if got := edgePoint(r, tt.dir, 5); !near(got, tt.want) {
			t.Errorf("edgePoint along %v = %v, want %v", tt.dir, got, tt.want)
		}
	}
}

func TestClickMarksAndUnmarks(t *testing.T) {
	m := &worldMap{bounds: rl.Rectangle{X: -1000, Y: -1000, Width: 2000, Height: 2000}}
	f := mapFrame{screen: rl.Rectangle{Width: 200, Height: 200}, scale: .5}
	click(m, f, rl.Vector2{X: 150, Y: 60}, 10)
	if !m.marked || !near(m.dest, rl.Vector2{X: 100, Y: -80}) {
		t.Fatalf("after a click: %v at %v, want marked at (100, -80)", m.marked, m.dest)
	}
	click(m, f, rl.Vector2{X: 30, Y: 30}, 10)
	if !m.marked || !near(m.dest, rl.Vector2{X: -140, Y: -140}) {
		t.Fatalf("a click elsewhere: %v at %v, want it moved to (-140, -140)", m.marked, m.dest)
	}
	click(m, f, rl.Vector2{X: 34, Y: 27}, 10)
	if m.marked {
		t.Fatal("a click on the mark left it marked")
	}
	f.at = rl.Vector2{X: 990}
	click(m, f, rl.Vector2{X: 190, Y: 100}, 10)
	if m.dest.X != 1000 {
		t.Errorf("marked off the world at %v", m.dest)
	}
}

func TestDistance(t *testing.T) {
	for m, want := range map[float32]string{7.4: "7 m", 99: "99 m", 344: "340 m", 996: "1.0 km", 2460: "2.5 km", 10250: "10.2 km"} {
		if got := distance(m); got != want {
			t.Errorf("distance(%v) = %q, want %q", m, got, want)
		}
	}
}

// On foot the body turns; in a seat, the root turns with the vehicle.
func TestFacingOf(t *testing.T) {
	east := rl.QuaternionFromAxisAngle(rl.Vector3{Y: 1}, math.Pi/2)
	if got := facingOf(rl.QuaternionIdentity(), east); !near(got, rl.Vector2{X: 1}) {
		t.Errorf("on foot, body turned east: %v", got)
	}
	if got := facingOf(east, rl.QuaternionIdentity()); !near(got, rl.Vector2{X: 1}) {
		t.Errorf("seated, root turned east: %v", got)
	}
	// A vehicle nosing down a slope still faces the way it's going.
	down := rl.QuaternionMultiply(east, rl.QuaternionFromAxisAngle(rl.Vector3{X: 1}, .3))
	if got := facingOf(down, rl.QuaternionIdentity()); !near(got, rl.Vector2{X: 1}) {
		t.Errorf("seated, nosing down a slope east: %v", got)
	}
}

func TestLookingFollowsTheDrawnCamera(t *testing.T) {
	v := &render.View3D{Active: true, Camera: rl.Camera3D{Position: rl.Vector3{X: 1, Y: 5, Z: 1}, Target: rl.Vector3{X: 4, Y: 1, Z: 1}}}
	if got := looking(v, rl.Vector3{Z: -1}); got.X <= 0 || math.Abs(float64(got.Z)) > 1e-5 {
		t.Errorf("looking = %v, want the drawn camera's east", got)
	}
	v.Active = false
	if got := looking(v, rl.Vector3{Z: -1}); got != (rl.Vector3{Z: -1}) {
		t.Errorf("no camera drawn: %v, want the steered way", got)
	}
}
