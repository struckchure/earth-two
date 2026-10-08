package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

type LifeState uint8

const (
	Healthy LifeState = iota
	Critical
	Dead
)

// Health is shared by players and NPCs. ImpactSpeed is the closing speed
// at impact, before the physics solver changes either body's velocity.
type Health struct {
	State       LifeState
	ImpactSpeed float32
	Vehicle     ecs.Entity
}

// Downed marks a fallen, finite-mass body, without a standing controller.
type Downed struct{}

const (
	ragdollRadius = float32(.20)
	ragdollHeight = float32(.65)
	ragdollOffset = float32(.12)
)

// KnockDown replaces the standing controller with a finite-mass torso and
// an articulated ragdoll. Keep the current orientation and animation pose;
// gravity and the impact make the body fall rather than snapping it sideways.
func KnockDown(root illusion.EntityCommands, at transform.Transform, health Health, velocity rl.Vector3) {
	// A strike low on the legs sweeps them ahead of the upper body (the
	// Hit On Legs reference), making the torso fall back into the impact.
	spin := rl.Vector3Scale(rl.Vector3CrossProduct(transform.Up, velocity), -.65)
	if speed := rl.Vector3Length(spin); speed > 5 {
		spin = rl.Vector3Scale(spin, 5/speed)
	}
	if rl.Vector3LengthSqr(spin) < .01 {
		spin = rl.Vector3{X: .8, Z: .3}
	}
	root.Remove(ecs.C[physics.CharacterController](), ecs.C[Seated](), ecs.C[rising]()).Insert(
		illusion.C(health), illusion.C(Downed{}), illusion.C(Ragdoll{velocity: velocity}), illusion.C(at),
		illusion.C(Intent{}), illusion.C(Traversal{}),
		illusion.C(physics.Dynamic), illusion.C(physics.Capsule(ragdollRadius, ragdollHeight).WithOffset(rl.Vector3{Y: ragdollOffset})),
		illusion.C(physics.CharacterBody{}),
		illusion.C(physics.Mass(70)), illusion.C(physics.Material{Friction: .6}),
		illusion.C(physics.Damping{Linear: .4, Angular: .8}),
		illusion.C(physics.ContinuousCollision{}),
		illusion.C(physics.Velocity{Linear: velocity, Angular: spin}),
	)
}

// Revive is prototype recovery: restore a healthy character at safe feet.
func Revive(cmd *illusion.Commands, e ecs.Entity, feet rl.Vector3) {
	center := rl.Vector3Add(feet, rl.Vector3{Y: capsuleHeight / 2})
	cmd.Entity(e).Remove(ecs.C[Downed](), ecs.C[Ragdoll](), ecs.C[physics.RigidBody](), ecs.C[physics.Collider](),
		ecs.C[physics.CharacterBody](),
		ecs.C[physics.Mass](), ecs.C[physics.Material](), ecs.C[physics.Damping](),
		ecs.C[physics.ContinuousCollision](), ecs.C[physics.Velocity]()).Insert(
		illusion.C(Health{}), illusion.C(Intent{}), illusion.C(Traversal{}),
		illusion.C(physics.CharacterController{Radius: capsuleRadius, Height: capsuleHeight, StepHeight: .3}),
		illusion.C(transform.FromTranslation(center)),
		illusion.C(MotionSamples{previous: center, current: center, height: capsuleHeight}),
	)
}

func poseDowned(
	bodies *illusion.Query4Where[State, render.Model3d, render.AnimationPlayer, transform.Transform, illusion.With[Body]],
	down *illusion.Query2Where[Ragdoll, transform.Transform, illusion.With[Downed]],
	models *illusion.Res[asset.Assets[render.Model]],
	anims *illusion.Res[asset.Assets[render.Animations]],
	hier *illusion.Hierarchy,
	fixed *illusion.Res[illusion.FixedTime],
	cmd *illusion.Commands,
) {
	bodies.Each(func(e ecs.Entity, st *State, m *render.Model3d, animation *render.AnimationPlayer, tr *transform.Transform) {
		root, ok := hier.Parent(e)
		if !ok {
			return
		}
		ragdoll, at, fallen := down.Get(root)
		if !fallen {
			if st.downed {
				*st = State{skin: st.skin, Current: st.Current}
				animation.Pose = nil
				animation.Paused, animation.ManualTime = false, false
				tr.Rotation = rl.QuaternionIdentity()
			}
			return
		}
		st.downed = true
		if st.hidden {
			st.hidden = false
			show(cmd, hier, e, true)
		}
		animation.Paused, animation.ManualTime = true, true
		model := models.Get().Get(m.Model)
		if model == nil {
			return
		}
		matrix := rl.MatrixMultiply(model.Transform, rl.MatrixMultiply(tr.Matrix(), at.Matrix()))
		if ragdoll.rig == nil {
			pose := animation.Pose
			if len(pose) == 0 {
				if clips := anims.Get().Get(animation.Animations); clips != nil {
					pose = animation.SamplePose(&model.Model, clips, nil)
				}
			}
			if len(pose) == 0 {
				pose = model.Skeleton.GetBindPose()
			}
			ragdoll.rig = newRagdollRig(model.Skeleton.GetBones(), pose, matrix, *at, ragdoll.velocity)
		}
		if ragdoll.rig != nil {
			animation.Pose = ragdoll.rig.pose(matrix, fixed.Get().Overstep())
		}
	})
}

func stopDowned(q *illusion.Query2[Intent, Downed]) {
	q.Each(func(_ ecs.Entity, in *Intent, _ *Downed) { *in = Intent{} })
}
