package game

import (
	"testing"

	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Compare against the actual placed entities: fixed lighting must include
// every authored lamp at its rotated, terrain-adjusted position.
func TestFixedLampDataMatchesWorldFixtures(t *testing.T) {
	lamps, err := fixedLamps("../assets")
	if err != nil {
		t.Fatal(err)
	}
	if len(lamps) <= render.MaxPointLights {
		t.Fatal("fixture must exercise more lamps than the camera budget")
	}
	want := map[shading.Lamp]int{}
	for _, l := range lamps {
		want[l]++
	}
	l := newLandfall(t, arrival)
	q := ecs.NewFilter2[render.PointLight, transform.GlobalTransform](l.app.World).Query()
	count := 0
	for q.Next() {
		light, pose := q.Get()
		lamp := shading.Lamp{At: pose.Translation(), Light: *light}
		if want[lamp] == 0 {
			t.Errorf("shader lamp does not match world fixture: %v", lamp)
		}
		want[lamp]--
		count++
	}
	if count != len(lamps) {
		t.Fatalf("shader has %d lamps, world has %d", len(lamps), count)
	}
	for l, n := range want {
		if n != 0 {
			t.Errorf("missing or duplicate fixture %v: %d", l, n)
		}
	}
}
