package game

import (
	"math"
	"slices"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
)

// TestZonesMatchTheShader: the sound changes where the light does: all
// outside out in the Fringe, all dome in the middle of the dome, all Hull
// in the Hull's lower deck, and part of each across the edges.
func TestZonesMatchTheShader(t *testing.T) {
	for _, c := range []struct {
		name                string
		at                  rl.Vector3
		outside, dome, hull float32
	}{
		{"the Pads", arrival, 1, 0, 0},
		{"the dome's middle", rl.Vector3{X: 20, Y: 2, Z: -30}, 0, 1, 0},
		{"the Hull's deck", rl.Vector3{X: -16, Y: 2, Z: 4}, 0, 0, 1},
	} {
		w := placesAt(c.at)
		if !close(w.outside, c.outside) || !close(w.dome, c.dome) || !close(w.hull, c.hull) {
			t.Errorf("%s: outside %.2f dome %.2f hull %.2f, want %.0f %.0f %.0f", c.name, w.outside, w.dome, w.hull, c.outside, c.dome, c.hull)
		}
	}
	// Across the dome's wall, a metre and a half in from its edge (half its
	// blend): partly each.
	edge := placesAt(rl.Vector3{X: 34 - 1.5, Y: 2, Z: -30})
	if edge.dome < .2 || edge.dome > .8 || !close(edge.outside+edge.dome+edge.hull, 1) {
		t.Errorf("at the dome's edge: %+v, want partly dome and adding up to 1", edge)
	}
}

func close(a, b float32) bool { return math.Abs(float64(a-b)) < .01 }

// TestWhatsUnderfoot: sand out on the dunes, paving on the Pads and the
// Hull's deck and Charter Row, soil on the road and in the dome, grating
// on kit, a rug where one's laid.
func TestWhatsUnderfoot(t *testing.T) {
	s := newSoundscape(&world.Kit{Pieces: map[string]world.Piece{}}, []world.Placement{
		{Piece: "charter_paving", At: [3]float32{-21, 0, -30}},
		{Piece: "rug", At: [3]float32{-20, 0, 10}},
		{Piece: "generator", At: [3]float32{-30, 0, 0}},
	})
	for _, c := range []struct {
		name  string
		at    rl.Vector3
		onKit bool
		want  footing
	}{
		{"the dunes", rl.Vector3{X: 6000, Z: 4000}, false, sand},
		{"the Pads", arrival, false, paving},
		{"the Hull's deck", rl.Vector3{X: -16, Z: 4}, false, paving},
		{"a catwalk", rl.Vector3{X: -16, Y: 4, Z: 4}, true, grating},
		{"Charter Row", rl.Vector3{X: -21.4, Z: -29.3}, false, paving},
		{"the caravan road", rl.Vector3{X: 2200, Z: 260}, false, soil},
		{"the dome's beds", rl.Vector3{X: 25, Z: 30}, false, soil},
		{"a rug", rl.Vector3{X: -19.5, Z: 10.4}, false, rug},
	} {
		if got := s.surfaceAt(c.at, c.onKit); got != c.want {
			t.Errorf("%s: %d underfoot, want %d", c.name, got, c.want)
		}
	}
	if at, d, ok := s.nearest("generator", rl.Vector3{X: -30, Y: 1, Z: 4}); !ok || d > 4.1 || at.X != -30 {
		t.Errorf("the nearest generator: %v %v %v", at, d, ok)
	}
}

// TestSoundsFallOffAndPan: a sound is full close up, gone past far, and
// heard on the side it's on.
func TestSoundsFallOffAndPan(t *testing.T) {
	ear := listener{at: rl.Vector3{}, right: rl.Vector3{X: 1}, set: true}
	if v, _ := ear.spatial(rl.Vector3{Z: 1}, 3, 30); v != 1 {
		t.Errorf("close up: volume %v, want 1", v)
	}
	if v, _ := ear.spatial(rl.Vector3{Z: 31}, 3, 30); v != 0 {
		t.Errorf("past far: volume %v, want 0", v)
	}
	mid, _ := ear.spatial(rl.Vector3{Z: 15}, 3, 30)
	if mid <= 0 || mid >= 1 {
		t.Errorf("between: volume %v, want between 0 and 1", mid)
	}
	if _, pan := ear.spatial(rl.Vector3{X: 10}, 3, 30); pan <= .5 {
		t.Errorf("on the right: pan %v, want well right", pan)
	}
	if _, pan := ear.spatial(rl.Vector3{X: -10}, 3, 30); pan >= -.5 {
		t.Errorf("on the left: pan %v, want well left", pan)
	}
}

// TestFeetStep: a foot rising and falling as in a walk steps once a stride,
// each time it comes down; standing still, or shuffling an inch, it
// doesn't.
func TestFeetStep(t *testing.T) {
	const dt = 1. / 60
	walk := func(f *foot, seconds, stride, lift float32) int {
		steps := 0
		for i := range int(seconds / dt) {
			phase := float64(i) * dt / float64(stride)
			// Planted for half the stride at the ankle's height, then up
			// and over.
			h := .09 + lift*float32(math.Max(0, math.Sin(2*math.Pi*phase)))
			if f.track(h, dt) {
				steps++
			}
		}
		return steps
	}
	if n := walk(&foot{}, 10, 1.1, .18); n < 8 || n > 10 {
		t.Errorf("walking 10 s at a stride a 1.1 s: %d steps, want 9 or so", n)
	}
	if n := walk(&foot{}, 10, .7, .3); n < 13 || n > 15 {
		t.Errorf("running 10 s at a stride a 0.7 s: %d steps, want 14 or so", n)
	}
	if n := walk(&foot{}, 10, 1, .02); n != 0 {
		t.Errorf("shuffling: %d steps, want none", n)
	}
}

// TestDustSettles: dust is thrown up, drifts and slows, and is gone once
// its life's out; there's never more than maxParticles of it.
func TestDustSettles(t *testing.T) {
	fx := newEffects()
	fx.ring(rl.Vector3{}, 20, 1, rl.White)
	if len(fx.ps) != 20 {
		t.Fatalf("%d motes thrown up, want 20", len(fx.ps))
	}
	for range 60 {
		fx.step(1. / 60)
	}
	for _, p := range fx.ps {
		if d := float32(math.Hypot(float64(p.pos.X), float64(p.pos.Z))); d < .3 {
			t.Fatalf("a mote's only %.2f m out after a second", d)
		}
	}
	for range 60 * 4 {
		fx.step(1. / 60)
	}
	if len(fx.ps) != 0 {
		t.Errorf("%d motes left after 5 s, want none", len(fx.ps))
	}
	for range maxParticles + 100 {
		fx.sparks(rl.Vector3{}, rl.Vector3{Y: -1}, 1)
	}
	if len(fx.ps) != maxParticles {
		t.Errorf("%d particles, want at most %d", len(fx.ps), maxParticles)
	}
}

// TestParticlesFaceTheCamera: each is drawn as a square of its size facing
// the camera, the farthest first.
func TestParticlesFaceTheCamera(t *testing.T) {
	fx := newEffects()
	fx.verts, fx.cols = make([]float32, 12*maxParticles), make([]uint8, 16*maxParticles)
	fx.add(particle{pos: rl.Vector3{Z: 5}, life: 1, size0: .5, size1: .5, colour: rl.White})
	fx.add(particle{pos: rl.Vector3{Z: 20}, life: 1, size0: .5, size1: .5, colour: rl.White})
	fx.add(particle{pos: rl.Vector3{Z: -5}, life: 1, size0: .5, size1: .5, colour: rl.White}) // behind
	n := fx.quads(rl.Vector3{}, rl.Vector3{Z: 1}, rl.Vector3{X: -1}, rl.Vector3{Y: 1})
	if n != 2 {
		t.Fatalf("%d drawn, want the 2 ahead", n)
	}
	if z := fx.verts[2]; z != 20 {
		t.Errorf("the first drawn is at z %v, want the farthest (20)", z)
	}
	// Its corners half a metre either side of it, across and up.
	if w, h := abs(fx.verts[3]-fx.verts[0]), abs(fx.verts[7]-fx.verts[4]); !close(w, 1) || !close(h, 1) {
		t.Errorf("a quad %v by %v, want 1 by 1", w, h)
	}
}

// TestVariantNames: a sound's variants are name_0, name_1, ...
func TestVariantNames(t *testing.T) {
	for name, want := range map[string]int{"step_sand_3": 9, "bell": -1, "ui_move_1": 7, "hull_hum": -1, "x_": -1} {
		if got := lastUnderscoreDigit(name); got != want {
			t.Errorf("%s: %d, want %d", name, got, want)
		}
	}
}

// TestEachVehicleHasItsOwnEngine: no two kinds of vehicle sound the same.
func TestEachVehicleHasItsOwnEngine(t *testing.T) {
	heard := map[string]string{}
	for kind := range vehicle.Handlings {
		e, ok := engineSound[kind]
		if !ok {
			t.Errorf("%s has no engine sound", kind)
			continue
		}
		if other, ok := heard[e.loop]; ok {
			t.Errorf("%s sounds like %s (%s)", kind, other, e.loop)
		}
		heard[e.loop] = kind
		if !slices.Contains(loopNames, e.loop) || !slices.Contains(engineLoops, e.loop) {
			t.Errorf("%s's engine %s isn't loaded or kept quiet", kind, e.loop)
		}
	}
}

// TestWhoSteps: walking, running, on stairs and crouched, feet step; in
// the air, sat, climbing or rolling, they don't.
func TestWhoSteps(t *testing.T) {
	for _, a := range []character.Anim{character.Idle, character.Walk, character.Run, character.StairsUp, character.StairsDown, character.Crouch} {
		if !stepping(a) {
			t.Errorf("%v: no steps, want them", a)
		}
	}
	for _, a := range []character.Anim{character.Jump, character.Fall, character.Roll, character.Slide, character.Vault, character.LadderClimb, character.Sitting, character.Drive, character.Punch, character.Fix} {
		if stepping(a) {
			t.Errorf("%v: steps, want none", a)
		}
	}
}
