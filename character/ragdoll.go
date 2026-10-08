package character

import (
	"math"
	"slices"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

const ragdollContactSkin float32 = .002

// Ragdoll combines a dynamic Jolt torso (which receives momentum from cars)
// with constrained particles for the limbs and head. The particles sweep the
// physics world, retaining bone lengths while gravity folds the joints.
// Keeping the rig on the root releases its buffers on revive or despawn.
type Ragdoll struct {
	velocity    rl.Vector3
	rig         *ragdollRig
	quiet       float32
	groundQuiet float32
}

type ragdollPoint struct {
	bone                      int
	position, previous, drawn rl.Vector3
	local                     rl.Vector3 // anchor in the torso's frame
	radius, weight            float32
}

type ragdollLink struct {
	a, b     int
	min, max float32
	bone     bool // bone links also drive the rendered skeleton
}

type ragdollRig struct {
	bones    []rl.BoneInfo
	rest     []rl.Transform
	scratch  []rl.Transform
	points   []ragdollPoint
	links    []ragdollLink
	root     transform.Transform
	stepTime float32
}

func newRagdollRig(bones []rl.BoneInfo, pose []rl.Transform, matrix rl.Matrix, root transform.Transform, velocity rl.Vector3) *ragdollRig {
	if len(bones) == 0 || len(pose) != len(bones) {
		return nil
	}
	r := &ragdollRig{bones: slices.Clone(bones), rest: slices.Clone(pose), scratch: make([]rl.Transform, len(pose)), root: root}
	indices := map[string]int{}
	for i, b := range bones {
		indices[boneName(b)] = i
	}
	add := func(name string, radius, weight float32) int {
		bone, ok := indices[name]
		if !ok {
			return -1
		}
		p := rl.Vector3Transform(pose[bone].Translation, matrix)
		r.points = append(r.points, ragdollPoint{bone: bone, position: p, previous: p, drawn: p,
			local: rl.Vector3Transform(p, rl.MatrixInvert(root.Matrix())), radius: radius, weight: weight})
		return len(r.points) - 1
	}
	link := func(a, b int) {
		length := rl.Vector3Distance(r.points[a].position, r.points[b].position)
		r.links = append(r.links, ragdollLink{a: a, b: b, min: length, max: length, bone: true})
	}
	// The torso supplies rigid anchors. Hands/fingers and feet/toes have
	// their own particles so palms and soles turn when they hit the ground.
	for _, chain := range [][]string{
		{"neck_01", "head"},
		{"upperarm_l", "lowerarm_l", "hand_l", "middle_03_l"},
		{"upperarm_r", "lowerarm_r", "hand_r", "middle_03_r"},
		{"thigh_l", "calf_l", "foot_l", "ball_l"},
		{"thigh_r", "calf_r", "foot_r", "ball_r"},
	} {
		var nodes []int
		for i, name := range chain {
			radius, weight := float32(.065), float32(1)
			if i == 0 {
				weight = 0
			}
			switch {
			case name == "head":
				radius = .13
			case chain[0] == "thigh_l" || chain[0] == "thigh_r":
				radius = .085
			}
			n := add(name, radius, weight)
			if n < 0 {
				break // partial/unsupported rigs keep their available bones
			}
			if len(nodes) > 0 {
				link(nodes[len(nodes)-1], n)
			}
			nodes = append(nodes, n)
		}
		if len(nodes) >= 3 {
			a, b := r.links[len(r.links)-(len(nodes)-1)].max, r.links[len(r.links)-(len(nodes)-2)].max
			// A lower distance bound limits folding; the upper bound keeps
			// elbows and knees short of a perfectly straight, unstable lock.
			r.links = append(r.links, ragdollLink{a: nodes[0], b: nodes[2], min: max(abs(a-b)+.01, (a+b)*.28), max: (a + b) * .995})
		}
	}
	// Seed Verlet momentum at the solver's substep duration. Root motion
	// supplies the anchors' momentum; limbs carry the same initial impact.
	r.stepTime = 1. / 120
	for i := range r.points {
		p := &r.points[i]
		momentum := velocity
		name := boneName(r.bones[p.bone])
		if name == "calf_l" || name == "calf_r" || name == "foot_l" || name == "foot_r" || name == "ball_l" || name == "ball_r" {
			momentum = rl.Vector3Scale(velocity, 1.25)
		}
		p.previous = rl.Vector3Subtract(p.position, rl.Vector3Scale(momentum, r.stepTime))
	}
	return r
}

func (r *ragdollRig) constrain(link ragdollLink) {
	a, b := &r.points[link.a], &r.points[link.b]
	delta := rl.Vector3Subtract(b.position, a.position)
	d := rl.Vector3Length(delta)
	w := a.weight + b.weight
	if d < .000001 || w == 0 {
		return
	}
	correction := rl.Vector3Scale(delta, (d-clamp(d, link.min, link.max))/(d*w))
	a.position = rl.Vector3Add(a.position, rl.Vector3Scale(correction, a.weight))
	b.position = rl.Vector3Subtract(b.position, rl.Vector3Scale(correction, b.weight))
}

type ragdollSweep func(from, delta rl.Vector3, radius float32) (physics.RayHit, bool)

// step uses fixed substeps, never render delta, so pausing and different
// frame rates do not change the fall. Collision projection follows every
// constraint pass, otherwise a joint correction could push a foot through
// a floor even after its initial movement had been swept.
func (r *ragdollRig) step(root transform.Transform, gravity rl.Vector3, dt float32, sweep ragdollSweep) {
	if dt <= 0 {
		return
	}
	for i := range r.points {
		r.points[i].drawn = r.points[i].position
	}
	steps := max(1, int(math.Ceil(float64(dt/(1./120)))))
	h := dt / float32(steps)
	for sub := range steps {
		t := float32(sub+1) / float32(steps)
		at := root
		at.Translation = rl.Vector3Lerp(r.root.Translation, root.Translation, t)
		at.Rotation = rl.QuaternionSlerp(r.root.Rotation, root.Rotation, t)
		for i := range r.points {
			p := &r.points[i]
			if p.weight == 0 {
				p.position = rl.Vector3Transform(p.local, at.Matrix())
				p.previous = p.position
				continue
			}
			motion := rl.Vector3Scale(rl.Vector3Subtract(p.position, p.previous), h/r.stepTime*float32(math.Exp(float64(-2*h))))
			p.previous = p.position
			p.position = rl.Vector3Add(p.position, rl.Vector3Add(motion, rl.Vector3Scale(gravity, h*h)))
		}
		for range 8 {
			for _, link := range r.links {
				r.constrain(link)
			}
			for i := range r.points {
				p := &r.points[i]
				if p.weight == 0 || sweep == nil {
					continue
				}
				delta := rl.Vector3Subtract(p.position, p.previous)
				if hit, ok := sweep(p.previous, delta, p.radius); ok {
					d := rl.Vector3Length(delta)
					if d > .000001 {
						p.position = rl.Vector3Add(p.previous, rl.Vector3Scale(delta, clamp(hit.Distance/d, 0, 1)))
						p.position = rl.Vector3Add(p.position, rl.Vector3Scale(hit.Normal, ragdollContactSkin))
					}
				}
			}
		}
		// Sliding contact loses energy, rather than bouncing forever.
		for i := range r.points {
			p := &r.points[i]
			if p.weight == 0 || sweep == nil {
				continue
			}
			if hit, ok := sweep(p.position, rl.Vector3{Y: -.008}, p.radius); ok {
				motion := rl.Vector3Subtract(p.position, p.previous)
				motion = rl.Vector3Subtract(motion, rl.Vector3Scale(hit.Normal, min(0, rl.Vector3DotProduct(motion, hit.Normal))))
				p.previous = rl.Vector3Subtract(p.position, rl.Vector3Scale(motion, .65))
			}
		}
		r.stepTime = h
	}
	r.root = root
}

func (r *ragdollRig) pose(matrix rl.Matrix, alpha float32) []rl.Transform {
	copy(r.scratch, r.rest)
	inverse := rl.MatrixInvert(matrix)
	for _, link := range r.links {
		if !link.bone {
			continue
		}
		a, b := r.points[link.a], r.points[link.b]
		point := func(p ragdollPoint) rl.Vector3 {
			at := p.position
			if p.weight != 0 {
				at = rl.Vector3Lerp(p.drawn, at, clamp(alpha, 0, 1))
			}
			return rl.Vector3Transform(at, inverse)
		}
		// Rotate branches instead of moving bones independently: joints,
		// fingers, clothing and attached accessories stay connected.
		from := rl.Vector3Subtract(r.scratch[b.bone].Translation, r.scratch[a.bone].Translation)
		to := rl.Vector3Subtract(point(b), point(a))
		rotateBranch(r.scratch, r.bones, a.bone, limbRotation(from, to))
	}
	return r.scratch
}

// The capsule supplies torso collision but has none of the rolling resistance
// that shoulders, clothes and limbs give a fallen person. Apply that resistance
// only once prone and touching solid ground, leaving the initial fall intact.
func resistRagdollRolling(e ecs.Entity, at *transform.Transform, velocity *physics.Velocity, mass float32, gravity rl.Vector3, world *physics.Physics, ragdoll *Ragdoll, dt float32) {
	quiet := ragdoll.groundQuiet
	ragdoll.groundQuiet = 0
	axis := rl.Vector3RotateByQuaternion(transform.Up, at.Rotation)
	center := rl.Vector3Add(at.Translation, rl.Vector3Scale(axis, ragdollOffset))
	for _, along := range []float32{0, ragdollHeight/2 - ragdollRadius, -(ragdollHeight/2 - ragdollRadius)} {
		from := rl.Vector3Add(center, rl.Vector3Scale(axis, along))
		hit, ok := world.CastRayExcluding(from, rl.Vector3{Y: -1}, (ragdollRadius+.02)/.82, e)
		if !ok || hit.Normal.Y < .82 || hit.Distance*hit.Normal.Y > ragdollRadius+.015 || !world.StaticSurface(hit.Entity) {
			continue
		}
		if abs(rl.Vector3DotProduct(axis, hit.Normal)) > .35 {
			return // let the torso finish toppling before resisting its roll
		}
		normalSpeed := rl.Vector3DotProduct(velocity.Linear, hit.Normal)
		if normalSpeed > .5 {
			return // lifted or thrown off the ground by a fresh impact
		}
		slide := rl.Vector3Subtract(velocity.Linear, rl.Vector3Scale(hit.Normal, normalSpeed))
		if rl.Vector3LengthSqr(velocity.Linear) < .08*.08 && rl.Vector3LengthSqr(velocity.Angular) < .2*.2 {
			ragdoll.groundQuiet = quiet + dt
			if ragdoll.groundQuiet >= .35 {
				world.Sleep(e)
				return
			}
		}
		force := rl.Vector3Scale(slide, -8*mass)
		// Static friction holds a quiet corpse on gentle slopes; otherwise
		// gravity continually replenishes the capsule's rolling motion.
		if rl.Vector3LengthSqr(slide) < .3*.3 && rl.Vector3LengthSqr(velocity.Angular) < 1.5*1.5 {
			downhill := rl.Vector3Subtract(gravity, rl.Vector3Scale(hit.Normal, rl.Vector3DotProduct(gravity, hit.Normal)))
			force = rl.Vector3Subtract(force, rl.Vector3Scale(downhill, mass))
		}
		world.AddForce(e, force)
		world.AddTorque(e, rl.Vector3Scale(velocity.Angular, -8*mass*ragdollRadius*ragdollRadius))
		return
	}
}

func stepRagdolls(q *illusion.Query4[Ragdoll, transform.Transform, physics.Velocity, physics.Mass], world *physics.Physics, settings *illusion.Res[physics.Settings], clock *illusion.Res[illusion.Time]) {
	if settings.Get().Paused {
		return
	}
	q.Each(func(e ecs.Entity, ragdoll *Ragdoll, at *transform.Transform, velocity *physics.Velocity, mass *physics.Mass) {
		dt := clock.Get().DeltaSecs()
		asleep := world.Asleep(e)
		if !asleep {
			ragdoll.quiet = 0
			resistRagdollRolling(e, at, velocity, float32(*mass), settings.Get().Gravity, world, ragdoll, dt)
			asleep = world.Asleep(e)
		}
		if ragdoll.rig == nil {
			return
		}
		if asleep && ragdoll.quiet >= .35 {
			return
		}
		ragdoll.rig.step(*at, settings.Get().Gravity, dt, func(from, delta rl.Vector3, radius float32) (physics.RayHit, bool) {
			return world.SweepCapsuleExcluding(from, delta, radius, 2*radius+ragdollContactSkin, e)
		})
		if asleep {
			for _, point := range ragdoll.rig.points {
				// Constraint passes can move a resting limb between opposite
				// sides of the contact skin. Allow that full range, otherwise
				// small solver differences (notably on x86) prevent sleep.
				const quietMotion = 2 * ragdollContactSkin
				if rl.Vector3DistanceSqr(point.position, point.drawn) > quietMotion*quietMotion {
					ragdoll.quiet = 0
					return
				}
			}
			ragdoll.quiet += dt
			if ragdoll.quiet >= .35 {
				for i := range ragdoll.rig.points {
					point := &ragdoll.rig.points[i]
					point.previous, point.drawn = point.position, point.position
				}
			}
		}
	})
}

// Death from any source takes the same path as vehicle knockdown. This also
// handles health changes to a seated character, which has no controller.
func fallIncapacitated(q *illusion.Query2Where[Health, transform.Transform, illusion.And[illusion.With[Character], illusion.Without[Downed]]], controllers *illusion.Query1[physics.CharacterController], velocities *illusion.Query1[physics.Velocity], cmd *illusion.Commands) {
	q.Each(func(e ecs.Entity, health *Health, at *transform.Transform) {
		if health.State == Healthy {
			return
		}
		velocity := rl.Vector3{}
		if cc, ok := controllers.Get(e); ok {
			velocity = cc.Velocity
		} else if v, ok := velocities.Get(e); ok {
			velocity = v.Linear
		}
		KnockDown(cmd.Entity(e), *at, *health, velocity)
	})
}
