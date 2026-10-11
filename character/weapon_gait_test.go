package character

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestWeaponGaitFollowsDirectionWithoutChangingTorsoOrLimbLengths(t *testing.T) {
	bones := make([]rl.BoneInfo, 4)
	for i, name := range []string{"spine_01", "thigh_l", "calf_l", "foot_l"} {
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
	}
	bones[0].Parent = -1
	bones[1].Parent = -1
	bones[2].Parent = 1
	bones[3].Parent = 2
	base := []rl.Transform{{Translation: rl.Vector3{Y: 1.2}}, {Translation: rl.Vector3{X: .1, Y: 1}}, {Translation: rl.Vector3{X: .1, Y: .6, Z: .2}}, {Translation: rl.Vector3{X: .1, Y: .18, Z: .2}}}
	for i := range base {
		base[i].Rotation = rl.QuaternionIdentity()
		base[i].Scale = rl.Vector3{X: 1, Y: 1, Z: 1}
	}
	for _, dir := range []rl.Vector3{{Z: 1}, {Z: -1}, {X: 1}, {X: -1}, {X: .7071, Z: -.7071}} {
		pose := append([]rl.Transform(nil), base...)
		weaponGait(pose, bones, dir)
		if pose[0] != base[0] {
			t.Fatal("directional gait modified the torso")
		}
		for i := 1; i < 3; i++ {
			if math.Abs(float64(rl.Vector3Distance(pose[i].Translation, pose[i+1].Translation)-rl.Vector3Distance(base[i].Translation, base[i+1].Translation))) > 1e-5 {
				t.Fatal("directional gait stretched a leg")
			}
		}
		want := rl.Vector3{X: .1 + .2*dir.X, Y: .18, Z: .2 * dir.Z}
		if rl.Vector3Distance(pose[3].Translation, want) > .001 {
			t.Fatalf("direction %v: foot %v, expected %v", dir, pose[3].Translation, want)
		}

		low := append([]rl.Transform(nil), base...)
		crouchGait(low, bones, dir)
		if low[3].Translation.X < .099 || low[0] != base[0] {
			t.Fatal("crouch step crossed the body or changed the torso")
		}
		for i := 1; i < 3; i++ {
			if math.Abs(float64(rl.Vector3Distance(low[i].Translation, low[i+1].Translation)-rl.Vector3Distance(base[i].Translation, base[i+1].Translation))) > 1e-5 {
				t.Fatal("crouch step stretched a leg")
			}
		}
	}
}
func TestHipFireMovementPenaltyReleases(t *testing.T) {
	normal := WeaponMovementSpeed(7, false, false)
	if WeaponMovementSpeed(7, false, true) != normal*.75 {
		t.Fatal("hip fire must slow walking and running")
	}
	if WeaponMovementSpeed(7, true, true) != MovementSpeed(7, true) {
		t.Fatal("hip-fire penalty stacks with ADS")
	}
}

func TestGaitClockPreservesPhaseAcrossStopsAndFrameRates(t *testing.T) {
	for _, fps := range []int{30, 60, 120} {
		phase := float32(.2)
		for i := 0; i < fps; i++ {
			phase = advanceGaitPhase(phase, .8, 1/float32(fps))
		}
		if math.Abs(float64(phase-.825)) > 1e-5 {
			t.Fatalf("%d fps phase %v", fps, phase)
		}
		if advanceGaitPhase(phase, 0, 1) != phase {
			t.Fatal("stopping reset the stride")
		}
	}
}
func TestGaitTransitionSmoothsLegsWithoutDelayingAim(t *testing.T) {
	bones := make([]rl.BoneInfo, 2)
	for i, name := range []string{"pelvis", "spine_01"} {
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
	}
	bones[0].Parent = -1
	bones[1].Parent = 0
	previous := []rl.Transform{{Rotation: rl.QuaternionIdentity()}, {Rotation: rl.QuaternionIdentity()}}
	target := []rl.Transform{{Translation: rl.Vector3{X: 1}, Rotation: rl.QuaternionIdentity()}, {Translation: rl.Vector3{X: 1}, Rotation: rl.QuaternionIdentity()}}
	smoothWeaponGait(target, previous, bones, 1.0/60)
	if target[0].Translation.X <= 0 || target[0].Translation.X >= .5 {
		t.Fatal("leg pose snapped on transition")
	}
	if target[1].Translation.X != 1 {
		t.Fatal("locomotion smoothing delayed the weapon torso")
	}
}

func TestCrouchMovementRespectsAnalogWeightAndADS(t *testing.T) {
	full := crouchVelocity(Intent{Move: rl.Vector3{X: 1}})
	half := crouchVelocity(Intent{Move: rl.Vector3{X: .5}})
	aim := crouchVelocity(Intent{Move: rl.Vector3{X: 1}, ADS: true, LoadKg: 8})
	if full.X != .8 || half.X != .4 || aim.X >= full.X*.55 {
		t.Fatalf("incorrect crouch speeds: full %v, half %v, ADS %v", full, half, aim)
	}
	if rl.Vector3Length(crouchVelocity(Intent{Move: rl.Vector3{X: 1, Z: 1}})) > .801 {
		t.Fatal("diagonal crouch exceeds speed limit")
	}
}
