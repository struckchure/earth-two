package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func TestIncapacitatedResidentsDoNotReviveWhenStreamed(t *testing.T) {
	for _, state := range []character.LifeState{character.Critical, character.Dead} {
		app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}, residentsPlugin{})
		t.Cleanup(app.Cleanup)
		app.InsertResource(illusion.R(&menu{}), illusion.R(&character.Roster{Skins: []character.Skin{{Scale: 1, Clips: makehuman}}}))
		app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
			cmd.Spawn(illusion.C(character.Player{}), illusion.C(transform.FromXYZ(0, .9, 0)))
			cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(800, 1, 800)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		}))
		rs := ecs.GetResource[residents](app.World)
		rs.add(resident{feet: rl.Vector3{X: 5}, home: rl.Vector3{X: 5}, left: 100})
		tick := func(n int) {
			for range n {
				app.Tick(time.Second / 60)
			}
		}
		tick(2)
		q := ecs.NewFilter2[residentOf, transform.Transform](app.World).Query()
		var actor ecs.Entity
		var pose transform.Transform
		for q.Next() {
			actor = q.Entity()
			_, tr := q.Get()
			pose = *tr
		}
		var cmd illusion.Commands
		cmd.InitParam(app.World)
		character.KnockDown(cmd.Entity(actor), pose, character.Health{State: state, ImpactSpeed: 12}, rl.Vector3{})
		tick(30)
		if rs.list[0].health.State != state {
			t.Fatal("injury was not recorded in resident data")
		}
		movePlayer := func(x float32) {
			q := ecs.NewFilter1[transform.Transform](app.World).With(ecs.C[character.Player]()).Query()
			for q.Next() {
				q.Get().Translation.X = x
			}
		}
		movePlayer(300)
		tick(2)
		if app.World.Alive(actor) {
			t.Fatal("distant fallen character was not released")
		}
		feet := rs.list[0].feet
		tick(60)
		if rs.list[0].feet != feet {
			t.Fatal("incapacitated resident kept walking as data")
		}
		movePlayer(0)
		tick(2)
		q2 := ecs.NewFilter2[residentOf, character.Health](app.World).Query()
		found := false
		for q2.Next() {
			found = true
			_, health := q2.Get()
			if health.State != state {
				t.Fatal("returning to the area revived a resident")
			}
			if ecs.NewMap[physics.CharacterController](app.World).Has(q2.Entity()) {
				t.Fatal("fallen resident was recreated with a standing blocker")
			}
			if !ecs.NewMap[character.Ragdoll](app.World).Has(q2.Entity()) {
				t.Fatal("returning fallen resident lost its ragdoll")
			}
			tr := ecs.NewMap[transform.Transform](app.World).Get(q2.Entity())
			up := rl.Vector3RotateByQuaternion(transform.Up, tr.Rotation)
			if up.Y > .2 || up.Y < -.2 {
				t.Fatal("returning fallen resident started upright at torso height")
			}
		}
		if !found {
			t.Fatal("fallen resident did not return to the active area")
		}
	}
}

func TestPlayerRecoveryRestoresHealthControllerAndCamera(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	t.Cleanup(app.Cleanup)
	keys := input.NewButtonInput[input.Key]()
	app.InsertResource(illusion.R(keys), illusion.R(&menu{}))
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		root := cmd.Spawn(illusion.C(character.Player{}), illusion.C(character.Default()))
		character.KnockDown(root, transform.FromXYZ(0, .9, 0), character.Health{State: character.Dead}, rl.Vector3{})
		cmd.Spawn(illusion.C(render.Camera3d{}), illusion.C(transform.FromXYZ(0, 3, 5)))
	}))
	app.AddSystems(illusion.Update, illusion.Fn5(recoverPlayer))
	app.Tick(time.Second / 60)
	keys.Press(rl.KeyR)
	app.Tick(time.Second / 60)
	q := ecs.NewFilter2[character.Health, transform.Transform](app.World).Query()
	for q.Next() {
		health, tr := q.Get()
		if health.State != character.Healthy {
			t.Fatal("R did not revive the player")
		}
		if flatDistance(tr.Translation, arrival) > .01 {
			t.Fatal("recovery did not return to the Pads")
		}
		if !ecs.NewMap[physics.CharacterController](app.World).Has(q.Entity()) || ecs.NewMap[physics.RigidBody](app.World).Has(q.Entity()) {
			t.Fatal("recovery left the fallen body active")
		}
	}
	if keys.JustPressed(rl.KeyR) {
		t.Fatal("recovery key leaked into roll input")
	}
	camera := ecs.NewFilter2[render.Camera3d, transform.Transform](app.World).Query()
	for camera.Next() {
		_, tr := camera.Get()
		if flatDistance(tr.Translation, arrival) > 6 {
			t.Fatal("camera was left behind during recovery")
		}
	}
}
