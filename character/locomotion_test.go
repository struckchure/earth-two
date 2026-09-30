package character

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/transform"
)

// turnAbout turns from 0 toward target at Default's walking turn, 60 times a
// second, returning how long it takes to get there and the fastest it went.
func turnAbout(t *testing.T, target float32) (secs, fastest float32) {
	c := Default()
	const dt = 1.0 / 60
	var yaw, rate float32
	for i := 1; i <= 600; i++ {
		prev := yaw
		yaw, rate = turnToward(yaw, rate, target, c.TurnSpeed, c.TurnAccel, dt)
		fastest = max(fastest, abs(rate))
		if d := abs(wrapAngle(yaw - prev)); d > c.TurnSpeed*dt+1e-4 {
			t.Fatalf("turned %.3f rad in one step, faster than TurnSpeed", d)
		}
		if yaw == target && rate == 0 {
			return float32(i) * dt, fastest
		}
	}
	t.Fatalf("never got to %.2f: at %.2f turning %.2f", target, yaw, rate)
	return 0, 0
}

func TestTurnAboutTakesAMoment(t *testing.T) {
	secs, fastest := turnAbout(t, math.Pi-0.01)
	if secs < 0.5 || secs > 0.9 {
		t.Errorf("turning about took %.2fs, want a walker's 0.5-0.9s", secs)
	}
	if fastest > Default().TurnSpeed+1e-3 {
		t.Errorf("turned at %.2f rad/s, over TurnSpeed", fastest)
	}
	small, _ := turnAbout(t, 0.3)
	if small >= secs/2 {
		t.Errorf("a slight turn took %.2fs, near a full turn's %.2fs", small, secs)
	}
}

func TestTurnTowardTakesTheShortWay(t *testing.T) {
	yaw, rate := turnToward(3, 0, -3, 7, 30, 1.0/60)
	if rate <= 0 || yaw <= 3 {
		t.Errorf("from 3 to -3 rad: yaw %.3f rate %.2f, want turning up through π", yaw, rate)
	}
}

func TestTurnTowardKeepsTurningWayAround(t *testing.T) {
	// Straight behind is as far either way; mid-turn, it keeps its way.
	yaw, rate := turnToward(0, -2, math.Pi, 7, 30, 1.0/60)
	if rate >= 0 || yaw >= 0 {
		t.Errorf("yaw %.3f rate %.2f, want still turning the negative way", yaw, rate)
	}
}

func TestStride(t *testing.T) {
	ahead := rl.Vector3{Z: 1}
	dir, share := stride(0, ahead)
	if share != 1 || rl.Vector3Distance(dir, ahead) > 1e-5 {
		t.Errorf("heading where it faces: %v %.2f, want straight on at full speed", dir, share)
	}
	dir, share = stride(0, rl.Vector3{Z: -1})
	if math.Abs(float64(share-crawl)) > 1e-5 {
		t.Errorf("heading straight back: share %.2f, want %.2f", share, crawl)
	}
	if a := math.Acos(float64(rl.Vector3DotProduct(dir, ahead))); a > slip+1e-4 {
		t.Errorf("heading straight back steps %.2f rad off its facing, more than slip", a)
	}
	_, side := stride(0, rl.Vector3{X: 1})
	if side <= crawl || side >= 1 {
		t.Errorf("heading sideways: share %.2f, want between crawl and 1", side)
	}
}

func TestYawOf(t *testing.T) {
	for _, yaw := range []float32{0, 1, -2, 3} {
		if got := yawOf(rl.QuaternionFromAxisAngle(transform.Up, yaw)); math.Abs(float64(got-yaw)) > 1e-4 {
			t.Errorf("yawOf(%.2f) = %.4f", yaw, got)
		}
	}
}

func TestApproach(t *testing.T) {
	got := approach(rl.Vector3{}, rl.Vector3{X: 3, Z: 4}, 1)
	if rl.Vector3Distance(got, rl.Vector3{X: 0.6, Z: 0.8}) > 1e-5 {
		t.Errorf("approach = %v, want one unit toward (3, 0, 4)", got)
	}
	if got := approach(rl.Vector3{X: 1}, rl.Vector3{X: 1.5}, 1); got.X != 1.5 {
		t.Errorf("approach within a step = %v, want there", got)
	}
}

func TestCanAct(t *testing.T) {
	still := motion{Grounded: true}
	if !canAct(still, false) {
		t.Error("standing still can't act")
	}
	if canAct(still, true) {
		t.Error("an action started over one still playing")
	}
	if !canAct(motion{Grounded: true, Speed: 4}, false) {
		t.Error("running can't act")
	}
	if canAct(motion{}, false) {
		t.Error("acted in the air")
	}
}

func TestAirborneKeepsToTheJump(t *testing.T) {
	launch := rl.Vector3{Z: 4}
	angle := func(v rl.Vector3) float64 { return math.Atan2(float64(v.X), float64(v.Z)) }

	side := airborne(launch, rl.Vector3{X: 4})
	if a := math.Abs(angle(side)); a > airSteer+1e-4 || a < airSteer-1e-4 {
		t.Errorf("steering sideways veered %.1f°, want %.1f°", a*180/math.Pi, airSteer*180/math.Pi)
	}
	if l := rl.Vector3Length(airborne(launch, rl.Vector3{Z: 9})); l > 4+1e-4 {
		t.Errorf("sped up in the air to %.2f", l)
	}
	if got := airborne(launch, rl.Vector3{}); got != launch {
		t.Errorf("letting go: %v, want to keep going %v", got, launch)
	}
	back := airborne(launch, rl.Vector3{Z: -4})
	if back.Z <= 0 || math.Abs(float64(back.Z-2)) > 1e-4 || math.Abs(float64(back.X)) > 1e-4 {
		t.Errorf("pulling back: %v, want still forward at half speed", back)
	}
	if got := airborne(rl.Vector3{}, rl.Vector3{X: 4}); got != (rl.Vector3{}) {
		t.Errorf("a standing jump drifted: %v", got)
	}
}
