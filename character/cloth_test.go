package character

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

// ring is n points round the Y axis at radius r and height y, centred on c.
func ring(c rl.Vector3, r, y float32, n int) []rl.Vector3 {
	out := make([]rl.Vector3, n)
	for i := range out {
		a := 2 * math.Pi * float64(i) / float64(n)
		out[i] = rl.Vector3Add(c, rl.Vector3{X: r * float32(math.Cos(a)), Y: y, Z: r * float32(math.Sin(a))})
	}
	return out
}

func TestFitCapsuleCentresOnTheBody(t *testing.T) {
	// A limb of radius 0.06 round x = 0, z = 0.1, its bone running up its
	// back edge at z = 0.05.
	var pts []rl.Vector3
	for _, y := range []float32{0.1, 0.3, 0.5} {
		pts = append(pts, ring(rl.Vector3{Z: 0.1}, 0.06, y, 32)...)
	}
	k, ok := fitCapsule(rl.Vector3{Z: 0.05}, rl.Vector3{Y: 0.6, Z: 0.05}, pts)
	if !ok {
		t.Fatal("no capsule")
	}
	if math.Abs(float64(k.A.Z-0.1)) > 1e-3 || math.Abs(float64(k.B.Z-0.1)) > 1e-3 {
		t.Errorf("ends %v %v, want them moved to z 0.1, the limb's middle", k.A, k.B)
	}
	if math.Abs(float64(k.Radius-0.06)) > 1e-3 {
		t.Errorf("radius %.4f, want the limb's 0.06", k.Radius)
	}
	if _, ok := fitCapsule(rl.Vector3{}, rl.Vector3{Y: 1}, nil); ok {
		t.Error("a capsule fitted to nothing")
	}
}

func TestBodyShapeNear(t *testing.T) {
	s := &bodyShape{grid: map[[3]int32][]rl.Vector3{}, cell: 0.03}
	for _, p := range []rl.Vector3{{}, {X: 0.5}} {
		k := s.key(p)
		s.grid[k] = append(s.grid[k], p)
	}
	for _, c := range []struct {
		p    rl.Vector3
		want bool
	}{
		{rl.Vector3{X: 0.01}, true},
		{rl.Vector3{X: -0.015, Y: 0.01}, true}, // across a cell edge
		{rl.Vector3{X: 0.03}, false},
		{rl.Vector3{X: 0.49, Z: 0.005}, true},
		{rl.Vector3{X: 0.25}, false},
	} {
		if got := s.near(c.p, 0.025); got != c.want {
			t.Errorf("near(%v) = %v, want %v", c.p, got, c.want)
		}
	}
}

func TestSplitSides(t *testing.T) {
	got := splitSides([]rl.Vector3{{X: -0.1}, {X: 0.2}, {X: 0.05}, {X: -0.3}}, 0.02)
	if len(got) != 2 || len(got[0]) != 2 || len(got[1]) != 2 || got[0][1].X != -0.3 || got[1][1].X != 0.05 {
		t.Errorf("split = %v", got)
	}
}
