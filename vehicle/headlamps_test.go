package vehicle

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func assertHeadlamps(t *testing.T, g *rig, want bool) {
	t.Helper()
	if got := get[Drivable](g, g.car).Headlamps; got != want {
		t.Fatalf("headlamps powered %v, want %v", got, want)
	}
	if g.driving.Active() && g.driving.Headlamps != want {
		t.Fatalf("HUD headlamps %v, want %v", g.driving.Headlamps, want)
	}
	count := 0
	for q := ecs.NewFilter2[Headlamp, shading.SpotLight](g.app.World).Query(); q.Next(); {
		_, l := q.Get()
		if l.Enabled != want {
			t.Errorf("beam enabled %v, want %v", l.Enabled, want)
		}
		count++
	}
	if count == 0 {
		t.Fatal("vehicle has no headlamps")
	}
}

func TestHeadlampsAutomaticAndManual(t *testing.T) {
	g := newRig(t)
	cycle := ecs.GetResource[LightCycle](g.app.World)
	assertHeadlamps(t, g, false)
	g.tap(rl.KeyH) // On foot: no vehicle controls.
	assertHeadlamps(t, g, false)
	cycle.Night = true
	g.tick(1)
	assertHeadlamps(t, g, false) // Parked vehicles do not automatically light up.
	g.tap(rl.KeyE)
	if !g.driving.Active() {
		t.Fatal("could not enter vehicle")
	}
	assertHeadlamps(t, g, true)
	cycle.Night = false
	g.tick(1)
	assertHeadlamps(t, g, false) // Dawn switches automatic lights off.
	cycle.Night = true
	g.tick(1)
	assertHeadlamps(t, g, true)
	g.tap(rl.KeyE)
	assertHeadlamps(t, g, false) // Automatic lamps switch off when exiting.
	g.tap(rl.KeyE)
	assertHeadlamps(t, g, true)
	g.tap(rl.KeyH)
	g.tick(120)
	assertHeadlamps(t, g, false) // Automation must not undo a manual off.
	g.tap(rl.KeyE)
	g.tap(rl.KeyE)
	assertHeadlamps(t, g, true) // A new drive returns to the automatic default.
	g.tap(rl.KeyH)
	g.tap(rl.KeyH) // Explicitly switch on.
	assertHeadlamps(t, g, true)
	g.tap(rl.KeyE)
	g.tick(120)
	assertHeadlamps(t, g, true) // Explicit on stays on after exiting.
	cycle.Night = false
	g.tick(1)
	assertHeadlamps(t, g, true) // The manual on remains authoritative by day.
	g.tap(rl.KeyH)
	assertHeadlamps(t, g, true) // On-foot H does not alter a parked vehicle.
	g.tap(rl.KeyE)
	g.tap(rl.KeyH)
	assertHeadlamps(t, g, false)
	g.tap(rl.KeyE)
	assertHeadlamps(t, g, false)
	g.tap(rl.KeyE)
	assertHeadlamps(t, g, false) // Entering by day leaves automatic lights off.
	// H can turn lights on by day, and holding H only toggles once.
	g.keys.Press(rl.KeyH)
	g.tick(1)
	g.keys.Clear()
	g.tick(60)
	g.keys.Release(rl.KeyH)
	g.keys.Clear()
	assertHeadlamps(t, g, true)
	controls := ecs.GetResource[character.Controls](g.app.World)
	controls.Enabled = false
	g.tap(rl.KeyH)
	assertHeadlamps(t, g, true)
}

func TestEveryRealVehicleHasForwardHeadlamps(t *testing.T) {
	for name, spec := range realSpecs(t) {
		t.Run(name, func(t *testing.T) {
			g := newRigWith(t, name, spec)
			tr := get[transform.Transform](g, g.car)
			tr.Translation = rl.Vector3{X: 12, Y: 4, Z: -7}
			tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, transform.Deg(90))
			g.tick(1)
			want := 2
			if spec.Handling == "bike" {
				want = 1
			}
			count := 0
			for q := ecs.NewFilter3[Headlamp, shading.SpotLight, transform.GlobalTransform](g.app.World).Query(); q.Next(); {
				_, l, p := q.Get()
				if p.Forward().X < .98 || p.Forward().Y >= 0 {
					t.Errorf("beam does not follow the nose and dip toward the road: %+v", p.Forward())
				}
				if p.Translation().X <= tr.Translation.X || l.Range < 30 {
					t.Errorf("lamp isn't ahead of the chassis or has too little reach: %+v, %+v", p.Translation(), l)
				}
				count++
			}
			if count != want {
				t.Fatalf("%d headlamps, want %d", count, want)
			}
		})
	}
}

func TestHOnlyChangesTheDrivenVehicle(t *testing.T) {
	g := newRig(t)
	var other ecs.Entity
	g.app.AddSystems(illusion.Update, illusion.Fn1(func(cmd *illusion.Commands) {
		if !other.IsZero() {
			return
		}
		none := func(asset.Handle[render.Model]) []illusion.Component { return nil }
		wheel := func(string) asset.Handle[render.Model] { return asset.Handle[render.Model]{} }
		car, err := Spawn(cmd, "other buggy", testBuggy(), asset.Handle[render.Model]{}, wheel, none, rl.Vector3{X: 20}, rl.QuaternionIdentity())
		if err != nil {
			t.Fatal(err)
		}
		car.Then(func(_ *ecs.World, e ecs.Entity) { other = e })
	}))
	g.tick(2)
	ecs.GetResource[LightCycle](g.app.World).Night = true
	get[Drivable](g, other).lightsMode = headlampsOn
	g.tick(1)
	g.tap(rl.KeyE)
	if !g.driving.Active() {
		t.Fatal("could not enter vehicle")
	}
	g.tap(rl.KeyH)
	if get[Drivable](g, g.car).Headlamps {
		t.Fatal("H didn't switch the driven car's lamps off")
	}
	if !get[Drivable](g, other).Headlamps {
		t.Fatal("H switched off another vehicle's lamps")
	}
}
