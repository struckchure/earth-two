package character

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestMotionSamplesPosition(t *testing.T) {
	s := MotionSamples{previous: rl.Vector3{X: 1, Y: 2}, current: rl.Vector3{X: 1.2, Y: 2.1}}
	for _, alpha := range []float32{0, 0.25, 0.5, 0.75, 1} {
		got := s.Position(s.current, alpha)
		want := rl.Vector3Lerp(s.previous, s.current, alpha)
		if rl.Vector3Distance(got, want) > 1e-5 {
			t.Errorf("alpha %v: position %v, want %v", alpha, got, want)
		}
	}
	if s.current != (rl.Vector3{X: 1.2, Y: 2.1}) {
		t.Error("presentation changed the physics position")
	}
	// A respawn between fixed steps must show at once.
	respawn := rl.Vector3{Y: 2}
	if got := s.Position(respawn, 0.5); got != respawn {
		t.Errorf("external move interpolated to %v, want %v", got, respawn)
	}
	// A teleport recorded by physics should also bypass interpolation.
	s.current = rl.Vector3{X: 20}
	if got := s.Position(s.current, 0.5); got != s.current {
		t.Errorf("teleport interpolated to %v", got)
	}
}

func TestCapsuleResizeInterpolationKeepsFeetPlanted(t *testing.T) {
	s := MotionSamples{previous: rl.Vector3{Y: .9}, current: rl.Vector3{Y: .9}, height: 1.8}
	for _, height := range []float32{.9, 1.8} {
		center := rl.Vector3{Y: height / 2}
		s.remember(center, height, true)
		for _, alpha := range []float32{0, .5, 1} {
			if y := s.Position(center, alpha).Y - height/2; abs(y) > .001 {
				t.Fatalf("resize to %v at alpha %v lifted feet by %v", height, alpha, y)
			}
		}
	}
}
