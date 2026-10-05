package game

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestViewSees(t *testing.T) {
	// From the origin looking down -Z, 45° tall and twice as wide.
	v := view{ahead: rl.Vector3{Z: -1}, up: rl.Vector3{Y: 1}, right: rl.Vector3{X: 1}, tanV: .4142, aspect: 2, far: drawDistance}
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
		if got := v.sees(tt.center, tt.radius); got != tt.want {
			t.Errorf("%s: sees = %v, want %v", tt.name, got, tt.want)
		}
	}
}
