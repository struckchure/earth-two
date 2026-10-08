package vehicle

import (
	"strconv"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

func pedestrian(g *rig, feet rl.Vector3, every int) ecs.Entity {
	var cmd illusion.Commands
	cmd.InitParam(g.app.World)
	var actor ecs.Entity
	roster := ecs.GetResource[character.Roster](g.app.World)
	roster.Spawn(&cmd, 0, feet,
		illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, StepHeight: .3, Every: every}),
	).Then(func(_ *ecs.World, e ecs.Entity) { actor = e })
	g.tick(30)
	return actor
}

func TestVehiclePushesPedestrianInsteadOfStopping(t *testing.T) {
	for _, drive := range []struct {
		name      string
		key       input.Key
		direction float32
	}{{"forward", rl.KeyW, 1}, {"reverse", rl.KeyS, -1}} {
		for _, every := range []int{1, 6, 8} {
			t.Run(drive.name+"/"+strconv.Itoa(every), func(t *testing.T) {
				g := newRig(t)
				g.tap(rl.KeyE)
				g.tick(60)
				start := get[transform.Transform](g, g.car).Translation
				person := pedestrian(g, rl.Vector3{X: start.X, Z: start.Z + 8*drive.direction}, every)
				before := get[transform.Transform](g, person).Translation
				g.keys.Press(drive.key)
				g.tick(240)
				car := get[transform.Transform](g, g.car).Translation
				after := get[transform.Transform](g, person).Translation
				speed := get[physics.VehicleState](g, g.car).Speed
				t.Logf("car=%v pedestrian=%v -> %v speed=%.2f", car, before, after, speed)
				if (car.Z-before.Z)*drive.direction < 4 || speed*drive.direction < 2 {
					t.Fatalf("a pedestrian stopped the car: car=%v pedestrian=%v speed=%.2f", car, after, speed)
				}
				if rl.Vector3Distance(before, after) < 2 {
					t.Fatal("the vehicle passed through the pedestrian without pushing them")
				}
			})
		}
	}
}

func TestPedestrianPhysicsKeepsCrowdLODOutsideCarContacts(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	near := pedestrian(g, rl.Vector3{X: 4}, 8)
	far := pedestrian(g, rl.Vector3{X: 50}, 8)
	farSkipped := false
	for range 8 {
		g.tick(1)
		close := get[physics.CharacterController](g, near)
		distant := get[physics.CharacterController](g, far)
		if !close.Stepped {
			t.Fatal("pedestrians within chassis reach must step every tick")
		}
		if close.Every != 8 || distant.Every != 8 {
			t.Fatal("the temporary override must preserve each pedestrian's configured interval")
		}
		farSkipped = farSkipped || !distant.Stepped
	}
	if !farSkipped {
		t.Fatal("distant pedestrians should retain their cheaper update rate")
	}
	// Pausing skips simulation, but must still undo the temporary override.
	ecs.GetResource[physics.Settings](g.app.World).Paused = true
	g.tick(1)
	if get[physics.CharacterController](g, near).Every != 8 {
		t.Fatal("pausing leaked the full-frequency override")
	}
}

func TestRealVehiclesKeepNormalPedestrianResponseWithCrowdLOD(t *testing.T) {
	for name, spec := range realSpecs(t) {
		t.Run(name, func(t *testing.T) {
			run := func(every int) (rl.Vector3, rl.Vector3, rl.Vector3) {
				g := newRigWith(t, name, spec)
				g.tap(rl.KeyE)
				if !g.driving.Active() {
					t.Fatal("could not enter the vehicle")
				}
				g.tick(60)
				start := get[transform.Transform](g, g.car).Translation
				person := pedestrian(g, rl.Vector3{X: start.X, Z: start.Z + spec.bounds().Max.Z + 6}, every)
				before := get[transform.Transform](g, person).Translation
				g.keys.Press(rl.KeyW)
				g.tick(300)
				car := get[transform.Transform](g, g.car).Translation
				after := get[transform.Transform](g, person).Translation
				return car, before, after
			}
			baseline, _, _ := run(1)
			car, before, after := run(6)
			// Crowd update frequency must not change contact response.
			// Vehicle balance is checked separately for every chassis.
			if rl.Vector3Distance(baseline, car) > .5 {
				t.Fatalf("crowd LOD changed %s's collision response: full-rate car=%v reduced-rate car=%v", name, baseline, car)
			}
			if rl.Vector3Distance(before, after) < .5 {
				t.Fatal("the pedestrian was not pushed on impact")
			}
		})
	}
}

func TestVehicleStillStopsAtSolidObstacles(t *testing.T) {
	for _, obstacle := range []string{"wall", "parked car"} {
		t.Run(obstacle, func(t *testing.T) {
			g := newRig(t)
			g.tap(rl.KeyE)
			var cmd illusion.Commands
			cmd.InitParam(g.app.World)
			if obstacle == "wall" {
				cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(8, 4, 1)), illusion.C(transform.FromXYZ(0, 2, 8)))
			} else {
				cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.ConvexHull(testBuggy().corners())), illusion.C(transform.FromXYZ(0, 0, 8)))
			}
			g.tick(2)
			g.keys.Press(rl.KeyW)
			g.tick(240)
			if at := get[transform.Transform](g, g.car).Translation; at.Z > 7 {
				t.Fatalf("vehicle drove through %s: %v", obstacle, at)
			}
		})
	}
}

func TestPedestrianCannotWalkThroughParkedVehicle(t *testing.T) {
	g := newRig(t)
	person := pedestrian(g, rl.Vector3{Z: 5}, 1)
	get[character.Intent](g, person).Move = rl.Vector3{Z: -1}
	g.tick(300)
	if at := get[transform.Transform](g, person).Translation; at.Z < 1.9 || at.Z > 4 {
		t.Fatalf("pedestrian should walk up to the parked chassis and stop: %v", at)
	}
}
