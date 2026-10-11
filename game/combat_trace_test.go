package game

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestBulletStreakMovesAndStopsAtObstruction(t *testing.T) {
	for _, kind := range []weaponKind{pistol, assaultRifle, sniperRifle, shotgun} {
		trace := bulletTrace{To: rl.Vector3{Z: 40}, Kind: kind}
		_, length, _ := tracerStyle(kind)
		previous := float32(0)
		for frame := 0; frame < 120; frame++ {
			trace.Age = float32(frame) / 240
			tail, head, alpha := trace.segment()
			if tail.Z < 0 || head.Z > 40 || tail.Z > head.Z || head.Z < previous {
				t.Fatalf("%v: invalid travel %v -> %v", kind, tail, head)
			}
			if rl.Vector3Distance(tail, head) > length+.001 {
				t.Fatal("streak became a full-length beam")
			}
			if alpha < 0 || alpha > 255 {
				t.Fatal("invalid fade")
			}
			previous = head.Z
		}
		if previous != 40 {
			t.Fatal("round did not reach the hit point")
		}
	}
}

func TestBulletStreamsOverlapExpireAndStayBounded(t *testing.T) {
	s := combatState{}
	s.addTrace(rl.Vector3{}, rl.Vector3{Z: 140}, assaultRifle)
	s.advanceTraces(.1)
	s.addTrace(rl.Vector3{}, rl.Vector3{Z: 100}, assaultRifle)
	if len(s.Traces) != 2 || s.Traces[0].Age != .1 || s.Traces[1].Age != 0 {
		t.Fatal("new shot replaced a travelling round")
	}
	for range 100 {
		s.addTrace(rl.Vector3{}, rl.Vector3{Z: 35}, shotgun)
	}
	if len(s.Traces) != maxBulletTraces {
		t.Fatal("unbounded visual stream")
	}
	s.advanceTraces(1)
	if len(s.Traces) != 0 {
		t.Fatal("expired tracers retained")
	}
}

func TestTracerTravelIsFrameRateIndependent(t *testing.T) {
	for _, fps := range []int{30, 60, 120} {
		s := combatState{}
		s.addTrace(rl.Vector3{}, rl.Vector3{Z: 300}, sniperRifle)
		for range fps / 10 {
			s.advanceTraces(1 / float32(fps))
		}
		_, head, _ := s.Traces[0].segment()
		if head.Z < 97.49 || head.Z > 97.51 {
			t.Fatalf("%d FPS: %v", fps, head)
		}
	}
}
