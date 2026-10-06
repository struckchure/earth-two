package vehicle

import (
	"encoding/json"
	"os"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// realSpecs are the drivable vehicles as world.json builds them.
func realSpecs(t *testing.T) map[string]*Spec {
	t.Helper()
	b, err := os.ReadFile("../assets/world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	var f struct {
		Pieces map[string]struct {
			Vehicle *Spec `json:"vehicle"`
		} `json:"pieces"`
	}
	if err := json.Unmarshal(b, &f); err != nil {
		t.Fatal(err)
	}
	out := map[string]*Spec{}
	for name, p := range f.Pieces {
		if p.Vehicle != nil {
			out[name] = p.Vehicle
		}
	}
	return out
}

// Every vehicle, as built, drives off, turns hard at speed and stays on its
// wheels.
func TestRealVehiclesStayUpright(t *testing.T) {
	for name, spec := range realSpecs(t) {
		t.Run(name, func(t *testing.T) {
			g := newRigWith(t, name, spec)
			g.tap(rl.KeyE)
			if !g.driving.Active() {
				t.Fatalf("couldn't get in: %+v", *g.prompt)
			}
			g.tick(60)
			g.keys.Press(rl.KeyW)
			worst := float32(1)
			for i := range 600 {
				if i == 240 {
					g.keys.Press(rl.KeyD)
				}
				if i == 420 {
					g.keys.Release(rl.KeyD)
					g.keys.Press(rl.KeyA)
				}
				g.tick(1)
				up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, get[transform.Transform](g, g.car).Rotation)
				worst = min(worst, up.Y)
			}
			st := get[physics.VehicleState](g, g.car)
			t.Logf("%s: speed %.1f m/s, worst up.Y %.2f", name, st.Speed, worst)
			if worst < 0.8 {
				t.Errorf("it went over (up.Y down to %.2f)", worst)
			}
		})
	}
}

// Sat on at a standstill, and ridden slowly, the bike stays up.
func TestBikeStaysUpSlow(t *testing.T) {
	g := newRigWith(t, "bike", realSpecs(t)["bike"])
	g.tap(rl.KeyE)
	worst := float32(1)
	track := func(n int) {
		for range n {
			g.tick(1)
			up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, get[transform.Transform](g, g.car).Rotation)
			worst = min(worst, up.Y)
		}
	}
	track(300)
	t.Logf("standing: worst up.Y %.2f", worst)
	g.keys.Press(rl.KeyW)
	track(40)
	g.keys.Release(rl.KeyW)
	track(240)
	t.Logf("crawling: worst up.Y %.2f, speed %.1f", worst, get[physics.VehicleState](g, g.car).Speed)
	if worst < 0.9 {
		t.Errorf("it fell over (up.Y down to %.2f)", worst)
	}
}
