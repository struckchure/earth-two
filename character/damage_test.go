package character

import (
	"github.com/mlange-42/ark/ecs"
	"testing"
)

func TestDamageTransitions(t *testing.T) {
	h := Health{}
	if h.Remaining() != 100 {
		t.Fatal("zero-value health must start full")
	}
	for range 4 {
		h.Strike(20, false, ecs.Entity{}, "Punch")
	}
	if h.State != Healthy || h.Remaining() != 20 {
		t.Fatalf("%+v", h)
	}
	h.Strike(20, false, ecs.Entity{}, "Punch")
	if h.State != Critical || h.Remaining() != 0 {
		t.Fatal("fists must be nonlethal")
	}
	h.Strike(34, true, ecs.Entity{}, "Pistol")
	if h.State != Dead {
		t.Fatal("lethal hit did not finish critical target")
	}
	if h.Strike(34, true, ecs.Entity{}, "Pistol") {
		t.Fatal("dead actor accepted damage")
	}
	h = Health{}
	if h.Strike(-10, true, ecs.Entity{}, "invalid") || h.Damage != 0 {
		t.Fatal("negative damage accepted")
	}
	for range 3 {
		h.Strike(34, true, ecs.Entity{}, "Pistol")
	}
	if h.State != Dead {
		t.Fatal("three pistol rounds must be lethal")
	}
}
