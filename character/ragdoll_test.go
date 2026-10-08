package character

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func ragdollTestSkeleton() ([]rl.BoneInfo, []rl.Transform) {
	names := []string{"upperarm_l", "lowerarm_l", "hand_l", "middle_03_l", "neck_01", "head"}
	positions := []rl.Vector3{{Y: 1.4}, {X: .35, Y: 1.35}, {X: .70, Y: 1.30}, {X: .80, Y: 1.30}, {Y: 1.5}, {Y: 1.68}}
	bones, pose := make([]rl.BoneInfo, len(names)), make([]rl.Transform, len(names))
	for i, name := range names {
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
		bones[i].Parent = int32(i - 1)
		if i == 4 {
			bones[i].Parent = -1
		}
		pose[i] = rl.Transform{Translation: positions[i], Rotation: rl.QuaternionIdentity(), Scale: rl.Vector3One()}
	}
	return bones, pose
}

func TestRagdollGravityFoldsLimbsWithoutStretchingSkeleton(t *testing.T) {
	bones, pose := ragdollTestSkeleton()
	for _, yaw := range []float32{0, .7, 2.4} {
		root := transform.FromXYZ(1, 2, 3).WithRotation(rl.QuaternionFromAxisAngle(transform.Up, yaw))
		matrix := root.Matrix()
		rig := newRagdollRig(bones, pose, matrix, root, rl.Vector3{})
		before := rig.points[3].position.Y
		for range 120 {
			rig.step(root, rl.Vector3{Y: -9.81}, 1./60, nil)
		}
		if rig.points[3].position.Y >= before-.15 {
			t.Fatal("arm remained frozen instead of falling under gravity")
		}
		for _, link := range rig.links {
			d := rl.Vector3Distance(rig.points[link.a].position, rig.points[link.b].position)
			if d < link.min-.008 || d > link.max+.008 {
				t.Fatalf("joint left its limits: distance %f, limits %f..%f", d, link.min, link.max)
			}
		}
		result := rig.pose(matrix, .5)
		for _, link := range rig.links {
			if !link.bone {
				continue
			}
			a, b := rig.points[link.a].bone, rig.points[link.b].bone
			if abs(rl.Vector3Distance(result[a].Translation, result[b].Translation)-rl.Vector3Distance(pose[a].Translation, pose[b].Translation)) > .0001 {
				t.Fatal("rendering stretched the skeleton")
			}
		}
	}
}

func TestRagdollSweepsFastLimbsAndSettlesOnGround(t *testing.T) {
	bones, pose := ragdollTestSkeleton()
	root := transform.Identity()
	rig := newRagdollRig(bones, pose, root.Matrix(), root, rl.Vector3{Z: 25, Y: -15})
	// Ground plus a thin wall: a fast limb must not cross either plane.
	sweep := func(from, delta rl.Vector3, radius float32) (physics.RayHit, bool) {
		fraction, normal := float32(2), rl.Vector3{}
		if delta.Y < 0 && from.Y+delta.Y < radius {
			fraction, normal = max(0, (radius-from.Y)/delta.Y), transform.Up
		}
		if delta.Z > 0 && from.Z+delta.Z > .15-radius {
			if f := max(0, (.15-radius-from.Z)/delta.Z); f < fraction {
				fraction, normal = f, rl.Vector3{Z: -1}
			}
		}
		return physics.RayHit{Distance: rl.Vector3Length(delta) * fraction, Normal: normal}, fraction <= 1
	}
	for range 300 {
		rig.step(root, rl.Vector3{Y: -9.81}, 1./60, sweep)
		for _, p := range rig.points {
			if p.weight > 0 && (p.position.Y < p.radius-.001 || p.position.Z > .15-p.radius+.001 || math.IsNaN(float64(p.position.X))) {
				t.Fatalf("ragdoll penetrated ground/wall or became unstable: %+v", p)
			}
		}
	}
}

func TestDeathStartsRagdollAndRecoveryClearsPoseAndPhysics(t *testing.T) {
	for _, seated := range []bool{false, true} {
		models, anims := asset.New[render.Model](nil), asset.New[render.Animations](nil)
		bones, pose := ragdollTestSkeleton()
		model := models.Add(render.Model{Model: rl.Model{Transform: rl.MatrixIdentity(), Skeleton: rl.ModelSkeleton{BoneCount: int32(len(bones)), Bones: &bones[0], BindPose: &pose[0]}}})
		app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}).InsertResource(illusion.R(models), illusion.R(anims))
		t.Cleanup(app.Cleanup)
		var actor ecs.Entity
		app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
			root := cmd.Spawn(illusion.C(Default()), illusion.C(Health{State: Dead}), illusion.C(transform.FromXYZ(0, .9, 0)))
			root.Then(func(_ *ecs.World, e ecs.Entity) { actor = e })
			if seated {
				root.Insert(illusion.C(Seated{}))
			} else {
				root.Insert(illusion.C(physics.CharacterController{Radius: .3, Height: 1.8}))
			}
			root.WithChild(illusion.C(Body{}), illusion.C(State{Current: Run}), illusion.C(render.Model3d{Model: model}), illusion.C(render.AnimationPlayer{Pose: pose}), illusion.C(transform.FromXYZ(0, -.9, 0)))
			cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(20, 1, 20)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		}))
		app.AddSystems(illusion.FixedPostUpdate, illusion.Fn4(fallIncapacitated).Before(physics.Prepare), illusion.Fn4(stepRagdolls).After(physics.Writeback))
		app.AddSystems(illusion.PostUpdate, illusion.Fn7(poseDowned).Before(transform.Propagate))
		app.Tick(time.Second / 60)
		if !ecs.NewMap[Ragdoll](app.World).Has(actor) || ecs.NewMap[physics.CharacterController](app.World).Has(actor) || ecs.NewMap[Seated](app.World).Has(actor) {
			t.Fatal("death did not replace the controller/seat with a ragdoll")
		}
		for range 60 {
			app.Tick(time.Second / 60)
		}
		rig := ecs.NewMap[Ragdoll](app.World).Get(actor).rig
		if rig == nil || len(rig.points) < 4 {
			t.Fatal("death did not initialize an articulated skeleton")
		}
		settings := ecs.GetResource[physics.Settings](app.World)
		settings.Paused = true
		before := rig.points[1].position
		app.Tick(time.Second / 30)
		if rig.points[1].position != before {
			t.Fatal("ragdoll kept moving while physics was paused")
		}
		settings.Paused = false
		var cmd illusion.Commands
		cmd.InitParam(app.World)
		Revive(&cmd, actor, rl.Vector3{X: 2})
		app.Tick(time.Second / 60)
		if ecs.NewMap[Ragdoll](app.World).Has(actor) || ecs.NewMap[physics.RigidBody](app.World).Has(actor) || !ecs.NewMap[physics.CharacterController](app.World).Has(actor) {
			t.Fatal("recovery retained ragdoll physics")
		}
		q := ecs.NewFilter2[State, render.AnimationPlayer](app.World).Query()
		for q.Next() {
			st, p := q.Get()
			if st.downed || p.Paused || p.ManualTime || len(p.Pose) != 0 {
				t.Fatal("recovery retained the ragdoll pose")
			}
		}
	}
}
