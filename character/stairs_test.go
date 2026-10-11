package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
	"testing"
	"time"
)

func TestStairGaitRequiresAuthoredSurface(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	defer app.Cleanup()
	var ground, body ecs.Entity
	velocity := float32(-1.6)
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(4, .2, 6)), illusion.C(transform.Identity().WithRotation(rl.QuaternionFromEuler(.57, 0, 0)))).Then(func(_ *ecs.World, e ecs.Entity) { ground = e })
		cmd.Spawn(illusion.C(physics.CharacterController{Radius: .3, Height: 1.8}), illusion.C(transform.FromXYZ(0, 1, 0))).WithChildren(func(children *illusion.ChildBuilder) {
			children.Spawn(illusion.C(Body{}), illusion.C(State{})).Then(func(_ *ecs.World, e ecs.Entity) { body = e })
		})
	}))
	app.AddSystems(illusion.Update, illusion.Chain(illusion.Fn1(func(roots *illusion.Query2[physics.CharacterController, transform.Transform]) {
		roots.Each(func(_ ecs.Entity, cc *physics.CharacterController, tr *transform.Transform) {
			tr.Translation = rl.Vector3{Y: 1}
			cc.Grounded = true
			cc.GroundNormal = rl.Vector3Normalize(rl.Vector3{Y: .84, Z: .54})
			cc.Velocity = rl.Vector3{Z: velocity}
		})
	}), illusion.Fn6(detectStairs)))
	app.Tick(time.Second / 60)
	states := ecs.NewMap[State](app.World)
	if states.Get(body).stairs != 0 {
		t.Fatal("ordinary sloped ground triggered the stair gait")
	}
	tags := ecs.NewMap[StairSurface](app.World)
	tags.Add(ground, &StairSurface{})
	app.Tick(time.Second / 60)
	if states.Get(body).stairs != 1 {
		t.Fatal("authored stairs did not trigger ascending gait")
	}
	velocity = 1.6
	app.Tick(time.Second / 60)
	if states.Get(body).stairs != -1 {
		t.Fatal("authored stairs did not trigger descending gait")
	}
	tags.Remove(ground)
	app.Tick(time.Second / 60)
	if states.Get(body).stairs != 0 || states.Get(body).stairsLeft != 0 {
		t.Fatal("stairs state leaked onto a natural slope")
	}
}
