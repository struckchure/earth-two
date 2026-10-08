package character

import (
	"fmt"
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

func fallenBody(t *testing.T, slope float32, start, velocity rl.Vector3) (*illusion.App, ecs.Entity, *physics.Physics) {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	t.Cleanup(app.Cleanup)
	var actor ecs.Entity
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		rotation := rl.QuaternionFromAxisAngle(rl.Vector3{Z: 1}, slope*math.Pi/180)
		floor := transform.Identity().WithRotation(rotation)
		floor.Translation = rl.Vector3RotateByQuaternion(rl.Vector3{Y: -.5}, rotation)
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(100, 1, 100)), illusion.C(floor))
		root := cmd.Spawn(illusion.C(Default()))
		root.Then(func(_ *ecs.World, e ecs.Entity) { actor = e })
		KnockDown(root, transform.FromTranslation(start), Health{State: Dead}, velocity)
	}))
	app.AddSystems(illusion.FixedPostUpdate, illusion.Fn4(stepRagdolls).After(physics.Writeback))
	app.Tick(time.Second / 60)
	world := &physics.Physics{}
	world.InitParam(app.World)
	return app, actor, world
}

func TestFallenBodiesSettleInsteadOfRollingAlongTheGround(t *testing.T) {
	for _, slope := range []float32{0, 5, 20} {
		for _, speed := range []float32{0, 12} {
			t.Run(fmt.Sprintf("slope=%g/impact=%g", slope, speed), func(t *testing.T) {
				app, actor, world := fallenBody(t, slope, rl.Vector3{Y: .9}, rl.Vector3{Z: speed})
				for range 360 {
					app.Tick(time.Second / 60)
				}
				before := *ecs.NewMap[transform.Transform](app.World).Get(actor)
				if !world.Asleep(actor) {
					t.Errorf("fallen body did not settle: velocity=%+v", *ecs.NewMap[physics.Velocity](app.World).Get(actor))
				}
				for range 300 {
					app.Tick(time.Second / 60)
				}
				after := *ecs.NewMap[transform.Transform](app.World).Get(actor)
				if rl.Vector3Distance(before.Translation, after.Translation) > .02 ||
					rl.Vector3Distance(rl.Vector3RotateByQuaternion(transform.Up, before.Rotation), rl.Vector3RotateByQuaternion(transform.Up, after.Rotation)) > .02 {
					t.Fatalf("corpse kept rolling after coming to rest: before=%+v after=%+v", before, after)
				}
				// Settling must not turn the corpse into an immovable obstacle.
				world.AddImpulse(actor, rl.Vector3{Z: 70 * 4, Y: 70 * 2})
				for range 30 {
					app.Tick(time.Second / 60)
				}
				if rl.Vector3Distance(after.Translation, ecs.NewMap[transform.Transform](app.World).Get(actor).Translation) < .2 {
					t.Fatal("settled body did not respond to a later impact")
				}
			})
		}
	}
}

func TestFallenBodyKeepsItsMomentumWhileAirborne(t *testing.T) {
	app, actor, _ := fallenBody(t, 0, rl.Vector3{Y: 20}, rl.Vector3{Z: 3})
	for range 60 {
		app.Tick(time.Second / 60)
	}
	velocity := ecs.NewMap[physics.Velocity](app.World).Get(actor)
	if velocity.Linear.Z < 1.8 || velocity.Linear.Y > -8 || rl.Vector3Length(velocity.Angular) < .3 {
		t.Fatalf("settling damped an airborne ragdoll: %+v", velocity)
	}
}

func TestFallenBodyCanStillSlideDownASteepSlope(t *testing.T) {
	app, actor, world := fallenBody(t, 45, rl.Vector3{Y: .9}, rl.Vector3{})
	for range 180 {
		app.Tick(time.Second / 60)
	}
	if world.Asleep(actor) || ecs.NewMap[transform.Transform](app.World).Get(actor).Translation.X > -1 {
		t.Fatal("settling pinned a corpse on a steep slope")
	}
}

func TestSettledRagdollLimbsStayStillAndWakeOnImpact(t *testing.T) {
	app, actor, world := fallenBody(t, 5, rl.Vector3{Y: .9}, rl.Vector3{Z: 4})
	bones, pose := ragdollTestSkeleton()
	at := *ecs.NewMap[transform.Transform](app.World).Get(actor)
	ragdoll := ecs.NewMap[Ragdoll](app.World).Get(actor)
	matrix := rl.MatrixMultiply(rl.MatrixTranslate(0, -.9, 0), at.Matrix())
	ragdoll.rig = newRagdollRig(bones, pose, matrix, at, rl.Vector3{Z: 4})
	for range 720 {
		app.Tick(time.Second / 60)
	}
	if !world.Asleep(actor) || ragdoll.quiet < .35 {
		motion := float32(0)
		for _, point := range ragdoll.rig.points {
			motion = max(motion, rl.Vector3Distance(point.position, point.drawn))
		}
		t.Fatalf("ragdoll did not settle: torso asleep=%t limb quiet time=%g max limb displacement=%g", world.Asleep(actor), ragdoll.quiet, motion)
	}
	before := make([]rl.Vector3, len(ragdoll.rig.points))
	for i, point := range ragdoll.rig.points {
		before[i] = point.position
	}
	for range 120 {
		app.Tick(time.Second / 60)
	}
	for i, point := range ragdoll.rig.points {
		if point.position != before[i] {
			t.Fatal("sleeping ragdoll limbs kept moving")
		}
	}
	world.AddImpulse(actor, rl.Vector3{Y: 70 * 2, Z: 70 * 4})
	for range 20 {
		app.Tick(time.Second / 60)
	}
	if ragdoll.quiet >= .35 || rl.Vector3Distance(before[0], ragdoll.rig.points[0].position) < .1 {
		t.Fatal("another impact did not wake the ragdoll skeleton")
	}
}
