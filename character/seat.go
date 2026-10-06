package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Seated is a character sitting in something that carries it, like a
// vehicle's seat. It has no capsule while it sits: [Sit] takes its
// physics.CharacterController away, so it can't collide with what carries
// it, and every system that walks, faces or animates a character leaves it
// alone. The sit system poses it at Pose instead, playing Anim.
//
// Whatever owns the seat writes Pose each frame, before the [Act] set runs:
// where the model's origin (its feet, standing) goes, and which way it faces.
type Seated struct {
	Seat ecs.Entity
	Pose transform.Transform
	Anim Anim
	// Hidden hides the sitter, clothes and all: shut in a cab, out of sight.
	Hidden bool
	// Hands and Feet are where the left and right hand and foot go, sat
	// astride (Ride): the grips and pegs, in Pose's frame. Without them they
	// go where a bike's usually are.
	Hands, Feet []rl.Vector3
}

// rising asks the sit system to get a character's body up, facing yaw.
type rising struct{ yaw float32 }

// Sit seats character e as s says. Whatever it was doing is dropped, a
// ladder or a vault included.
func Sit(cmd *illusion.Commands, e ecs.Entity, s Seated) {
	cmd.Entity(e).
		Remove(ecs.C[physics.CharacterController](), ecs.C[rising]()).
		Insert(illusion.C(s), illusion.C(Intent{}), illusion.C(Traversal{}))
}

// Stand gets seated character e up, standing at feet and facing yaw (radians
// from +Z toward +X). Feet should have room for a standing capsule.
func Stand(cmd *illusion.Commands, e ecs.Entity, feet rl.Vector3, yaw float32) {
	cmd.Entity(e).
		Remove(ecs.C[Seated]()).
		Insert(
			illusion.C(physics.CharacterController{Radius: capsuleRadius, Height: capsuleHeight, StepHeight: 0.3}),
			illusion.C(transform.FromTranslation(rl.Vector3Add(feet, rl.Vector3{Y: capsuleHeight / 2}))),
			illusion.C(Intent{}), illusion.C(Traversal{}), illusion.C(rising{yaw: yaw}),
		)
}

// sit poses seated characters at their seats, playing their seated clip,
// and gets characters that have stood up back on their feet.
func sit(
	bodies *illusion.Query3Where[transform.Transform, State, render.AnimationPlayer, illusion.With[Body]],
	seated *illusion.Query2[Seated, transform.Transform],
	getUp *illusion.Query2[rising, transform.Transform],
	hier *illusion.Hierarchy,
	sk *skins,
	controls *illusion.Res[Controls],
	settings *illusion.Res[physics.Settings],
	cmd *illusion.Commands,
) {
	skins := sk.roster.Get().Skins
	bodies.Each(func(e ecs.Entity, tr *transform.Transform, st *State, p *render.AnimationPlayer) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		skin := &skins[st.skin]
		if s, root, ok := seated.Get(parent); ok {
			up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: capsuleHeight / 2}, s.Pose.Rotation)
			root.Translation = rl.Vector3Add(s.Pose.Translation, up)
			root.Rotation = s.Pose.Rotation
			tr.Translation = rl.Vector3{Y: -capsuleHeight / 2}
			tr.Rotation = rl.QuaternionIdentity()
			st.turn, st.air, st.aloft, st.stairs, st.stairsLeft = 0, 0, false, 0, 0
			p.ManualTime = false
			p.Speed = 1
			if s.Anim != st.Current {
				st.Current = s.Anim
				clip := skin.clip(s.Anim)
				if s.Anim == Ride && !skin.Has(Ride) {
					clip = skin.clip(Drive) // no clip astride: sit as at a wheel
				}
				play(p, clip, false)
				p.FadeIn(.3)
			}
			p.Paused = !controls.Get().Enabled || settings.Get().Paused
			if s.Hidden != st.hidden {
				st.hidden = s.Hidden
				show(cmd, hier, e, !s.Hidden)
			}
			return
		}
		if r, root, ok := getUp.Get(parent); ok {
			root.Rotation = rl.QuaternionIdentity()
			tr.Translation = rl.Vector3{Y: -capsuleHeight / 2}
			tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, r.yaw)
			st.Current = Idle
			play(p, skin.clip(Idle), false)
			if st.hidden {
				st.hidden = false
				show(cmd, hier, e, true)
			}
			p.FadeIn(.3)
			cmd.Entity(parent).Remove(ecs.C[rising]())
		}
	})
}

// show shows or hides a body and everything on it.
func show(cmd *illusion.Commands, hier *illusion.Hierarchy, body ecs.Entity, visible bool) {
	set := func(e ecs.Entity) {
		if visible {
			cmd.Entity(e).Remove(ecs.C[render.Hidden]())
		} else {
			cmd.Entity(e).Insert(illusion.C(render.Hidden{}))
		}
	}
	set(body)
	hier.EachDescendant(body, set)
}
