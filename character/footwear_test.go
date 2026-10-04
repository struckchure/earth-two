package character

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestShoeUpperFollowsTongueAndToe(t *testing.T) {
	var points []rl.Vector3
	for i := range 25 {
		z := float32(i) * 0.01
		top := float32(0.10) - z*0.2
		points = append(points, rl.Vector3{X: -0.05, Y: 0.01, Z: z}, rl.Vector3{X: 0.05, Y: 0.01, Z: z}, rl.Vector3{Y: top, Z: z})
	}
	caps := fitShoeUpper(points, 7)
	if len(caps) != 6 {
		t.Fatalf("%d pieces, want 6", len(caps))
	}
	for _, c := range caps {
		if c.BoneA != 7 || c.BoneB != 7 {
			t.Fatal("shoe did not follow its bone")
		}
		if math.Abs(float64(c.Radius-0.05)) > 1e-5 {
			t.Fatalf("shoe width = %v", c.Radius)
		}
	}
	if caps[0].A.Y+caps[0].Radius <= caps[5].A.Y+caps[5].Radius {
		t.Fatal("toe inflated to tongue height")
	}
	// A cuff at tongue height can move onto it. A short hem cannot.
	if f := cuffClearance(rl.Vector3{Y: 0.08, Z: 0.02}, caps); f <= 0 {
		t.Fatal("cuff remained pinned inside tongue")
	}
	if f := cuffClearance(rl.Vector3{Y: 0.4, Z: 0.02}, caps); f != 0 {
		t.Fatal("shoe changed shorts")
	}
	if f := cuffClearance(rl.Vector3{Y: 0.08, Z: 0.02}, nil); f != 0 {
		t.Fatal("bare feet changed cuff freedom")
	}
}
