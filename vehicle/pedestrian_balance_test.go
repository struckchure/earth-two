package vehicle

import (
	"fmt"
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// Compare vehicle tilt throughout the impact with an empty road. Checking
// only the final rotation misses a vehicle that tips and later recovers.
func TestCharacterContactsCannotTipVehicles(t *testing.T) {
	for name, spec := range realSpecs(t) {
		for _, speed := range []float32{2, 12, -2, -12} {
			for _, body := range []string{"small", "large", "fallen"} {
				t.Run(fmt.Sprintf("%s/%s/%g", name, body, speed), func(t *testing.T) {
					baseline, occupied := newRigWith(t, name, spec), newRigWith(t, name, spec)
					for _, g := range []*rig{baseline, occupied} {
						g.tap(rl.KeyE)
						if !g.driving.Active() {
							t.Fatal("could not enter vehicle")
						}
						g.tick(60)
					}
					var cmd illusion.Commands
					cmd.InitParam(occupied.app.World)
					direction, edge := float32(1), spec.bounds().Max.Z
					if speed < 0 {
						direction, edge = -1, spec.bounds().Min.Z
					}
					radius, height := float32(.1), max(float32(.9), 2*(spec.bounds().Min.Y+.1))
					if body == "large" {
						radius, height = 1.8, 8
					}
					// Off-centre contacts are especially prone to rolling a bike or
					// trike. Fallen bodies also sit under a wheel's suspension cast.
					x := min(spec.Wheels[0].At[0], spec.bounds().Max.X-.1)
					at := transform.FromXYZ(x, height/2+.025, edge+direction*(radius+.12))
					var person ecs.Entity
					root := cmd.Spawn(illusion.C(character.Character{}), illusion.C(character.Health{}), illusion.C(at))
					if body == "fallen" {
						at.Translation.Y = spec.bounds().Min.Y + .25
						root.Insert(illusion.C(at), illusion.C(character.Downed{}), illusion.C(physics.Dynamic),
							illusion.C(physics.Capsule(.15, .5)), illusion.C(physics.CharacterBody{}),
							illusion.C(physics.Mass(7000)), illusion.C(physics.Velocity{}))
					} else {
						root.Insert(illusion.C(physics.CharacterController{Radius: radius, Height: height, Every: 8}))
					}
					root.Then(func(_ *ecs.World, e ecs.Entity) { person = e })
					baseline.tick(2)
					occupied.tick(2)
					before := get[transform.Transform](occupied, person).Translation
					for _, g := range []*rig{baseline, occupied} {
						get[physics.Velocity](g, g.car).Linear = rl.Vector3{Z: speed}
					}
					for frame := range 120 {
						baseline.tick(1)
						occupied.tick(1)
						empty := get[transform.Transform](baseline, baseline.car)
						car := get[transform.Transform](occupied, occupied.car)
						up := rl.Vector3RotateByQuaternion(transform.Up, car.Rotation)
						if math.IsNaN(float64(up.Y)) || up.Y < .8 {
							t.Fatalf("character tipped vehicle at frame %d: up=%v", frame, up)
						}
						if rl.Vector3Distance(up, rl.Vector3RotateByQuaternion(transform.Up, empty.Rotation)) > .03 {
							t.Fatalf("character changed vehicle tilt at frame %d: empty=%+v occupied=%+v", frame, empty, car)
						}
					}
					after := get[transform.Transform](occupied, person).Translation
					if rl.Vector2Distance(rl.Vector2{X: before.X, Y: before.Z}, rl.Vector2{X: after.X, Y: after.Z}) < .1 {
						t.Fatal("vehicle passed through character without pushing them")
					}
				})
			}
		}
	}
}
