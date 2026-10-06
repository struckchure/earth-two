package game

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestViewSees(t *testing.T) {
	// From the origin looking down -Z, 45° tall and twice as wide.
	v := view{ahead: rl.Vector3{Z: -1}, up: rl.Vector3{Y: 1}, right: rl.Vector3{X: 1}, tanV: .4142, aspect: 2}
	for _, tt := range []struct {
		name   string
		center rl.Vector3
		radius float32
		want   bool
	}{
		{"straight ahead", rl.Vector3{Z: -10}, 1, true},
		{"behind", rl.Vector3{Z: 10}, 1, false},
		{"behind, but big enough to reach round", rl.Vector3{Z: 3}, 5, true},
		{"off to the side", rl.Vector3{X: 30, Z: -10}, 1, false},
		{"just in, at the side", rl.Vector3{X: 8, Z: -10}, 1, true},
		{"overhead", rl.Vector3{Y: 30, Z: -10}, 1, false},
		{"too far", rl.Vector3{Z: -drawDistance - 5}, 1, false},
		{"far, but big enough to reach in", rl.Vector3{Z: -drawDistance - 5}, 10, true},
	} {
		if got := v.sees(tt.center, tt.radius, drawDistance); got != tt.want {
			t.Errorf("%s: sees = %v, want %v", tt.name, got, tt.want)
		}
	}
}

func TestBigPiecesAreSeenFurther(t *testing.T) {
	if sight(.8) > drawDistance+20 {
		t.Errorf("a crate is drawn %v m away, want about %v", sight(.8), drawDistance)
	}
	if sight(24) < 9000 {
		t.Errorf("the bow is drawn only %v m away, want it seen from the Pads (10 km)", sight(24))
	}
	if sight(1000) != sightMax {
		t.Errorf("sight caps at %v, want %v", sight(1000), sightMax)
	}
}

func TestDetailChunkBounds(t *testing.T) {
	at := rl.Vector3{X: 9800, Z: -1800}
	b := chunkSphere(at)
	for _, x := range []float32{-chunkSize / 2, chunkSize / 2} {
		for _, z := range []float32{-chunkSize / 2, chunkSize / 2} {
			for _, height := range []float32{-66, 200} {
				corner := rl.Vector3Add(at, rl.Vector3{X: x, Y: height, Z: z})
				if rl.Vector3Distance(corner, b.center) > b.radius {
					t.Fatalf("chunk bounds clip terrain corner %v", corner)
				}
			}
		}
	}
	v := view{ahead: rl.Vector3{Z: -1}, up: rl.Vector3{Y: 1}, right: rl.Vector3{X: 1}, tanV: .4142, aspect: 2}
	v.prepare()
	for _, tt := range []struct {
		z       float32
		visible bool
	}{{-chunkSize, true}, {chunkSize, false}} {
		b := chunkSphere(rl.Vector3{Z: tt.z})
		if got := v.sees(b.center, b.radius, clipFar); got != tt.visible {
			t.Errorf("chunk at Z=%v: visible %v, want %v", tt.z, got, tt.visible)
		}
	}
}

func BenchmarkViewSees(b *testing.B) {
	v := view{ahead: rl.Vector3{Z: -1}, up: rl.Vector3{Y: 1}, right: rl.Vector3{X: 1}, tanV: .4142, aspect: 2}
	v.prepare()
	for i := 0; i < b.N; i++ {
		v.sees(rl.Vector3{X: float32(i % 100), Z: -100}, 4, drawDistance)
	}
}
