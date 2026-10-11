package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"testing"
)

func TestWeaponLayerKeepsMovingLegsAndPelvis(t *testing.T) {
	bones := make([]rl.BoneInfo, 5)
	for i, name := range []string{"pelvis", "spine_01", "hand_r", "thigh_l", "foot_l"} {
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
	}
	bones[0].Parent = -1
	bones[1].Parent = 0
	bones[2].Parent = 1
	bones[3].Parent = 0
	bones[4].Parent = 3
	base, upper := make([]rl.Transform, 5), make([]rl.Transform, 5)
	for i := range base {
		base[i].Translation = rl.Vector3{X: float32(i), Y: 2}
		upper[i].Translation = rl.Vector3{Z: float32(i), Y: 1}
	}
	before := append([]rl.Transform(nil), base...)
	layerUpperBody(base, upper, bones)
	for _, i := range []int{0, 3, 4} {
		if base[i] != before[i] {
			t.Fatalf("locomotion bone %d overwritten", i)
		}
	}
	if base[2].Translation != (rl.Vector3{Y: 2, Z: 2}) {
		t.Fatalf("weapon hand did not follow pelvis: %+v", base[2])
	}
}

func TestEquipmentLoadSlowsWithoutChangingUnloadedMovement(t *testing.T) {
	if LoadSpeed(0) != 1 || LoadSpeed(-5) != 1 {
		t.Fatal("unloaded movement changed")
	}
	if !(LoadSpeed(2) > LoadSpeed(7) && LoadSpeed(7) > LoadSpeed(14)) {
		t.Fatal("heavier load must slow the character further")
	}
	if LoadSpeed(100) != .65 {
		t.Fatal("heavy-load movement became unplayable")
	}
}

func TestADSSlowsWeightedMovementAndReleases(t *testing.T) {
	for _, kg := range []float32{0, 5, 12} {
		normal := MovementSpeed(kg, false)
		aimed := MovementSpeed(kg, true)
		if aimed >= normal*.6 || aimed <= normal*.5 {
			t.Fatalf("ADS did not reduce load %.1f: %f to %f", kg, normal, aimed)
		}
		if MovementSpeed(kg, false) != LoadSpeed(kg) {
			t.Fatal("releasing ADS retained a movement penalty")
		}
	}
}
