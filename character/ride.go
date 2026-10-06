package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Riding astride (a bike, a trike) has no clip of its own: it's the driving
// clip (sat at a wheel, thighs level, feet forward on pedals) reshaped. The
// body leans forward over the tank just as far as it has to for its hands to
// reach the grips, which they take palms down, thumbs in, and the feet go
// back on the pegs under the hips. A seat without grips or pegs has them
// where a bike's usually are: relative to the hips, in the model's frame
// (facing +Z, its left +X), in metres.
var (
	ridePeg = rl.Vector3{X: .19, Y: -.47, Z: .12}
	rideBar = rl.Vector3{X: .3, Y: -.1, Z: .68}
)

const (
	rideLeanLeast = 12 * rl.Deg2rad // even when the grips are close
	rideLeanMost  = 55 * rl.Deg2rad
	rideReach     = .96 // the share of an arm's length it stretches to
	gripAhead     = .07 // how far ahead of the wrist the bar lies in the hand
	gripBelow     = .035
	ankleAbovePeg = .08
)

// rideLimb is an arm or a leg: its three bones, which side it's on (+1 left,
// -1 right), and for a hand, the bones that give its palm's frame.
type rideLimb struct {
	upper, middle, end int
	side               float32
	arm                bool
	fingers, index     int // middle_01 and index_01, or -1
	pinky              int // pinky_01, or -1
}

// rideRig is a model's riding limbs and spine, and the pose being shaped.
type rideRig struct {
	model         render.Model3d
	pelvis, spine int
	limbs         []rideLimb
	pose          []rl.Transform
}

type rideCache struct {
	rigs   map[ecs.Entity]*rideRig
	posing map[ecs.Entity]bool
}

func makeRideRig(m render.Model3d, bones []rl.BoneInfo) *rideRig {
	at := map[string]int{}
	for i, b := range bones {
		at[boneName(b)] = i
	}
	find := func(name string) int {
		if i, ok := at[name]; ok {
			return i
		}
		return -1
	}
	r := &rideRig{model: m, pelvis: find("pelvis"), spine: find("spine_01")}
	for _, side := range []struct {
		name string
		x    float32
	}{{"l", 1}, {"r", -1}} {
		for _, arm := range []bool{true, false} {
			names := [3]string{"thigh_", "calf_", "foot_"}
			if arm {
				names = [3]string{"upperarm_", "lowerarm_", "hand_"}
			}
			l := rideLimb{upper: find(names[0] + side.name), middle: find(names[1] + side.name), end: find(names[2] + side.name),
				side: side.x, arm: arm, fingers: -1, index: -1, pinky: -1}
			if l.upper < 0 || l.middle < 0 || l.end < 0 {
				continue
			}
			if arm {
				l.fingers, l.index, l.pinky = find("middle_01_"+side.name), find("index_01_"+side.name), find("pinky_01_"+side.name)
			}
			r.limbs = append(r.limbs, l)
		}
	}
	return r
}

// ride shapes the pose of characters seated astride: Ride, for a skin with no
// clip for it, playing the driving clip in its stead.
func ride(
	bodies *illusion.Query4Where[State, render.Model3d, render.AnimationPlayer, transform.Transform, illusion.With[Body]],
	seated *illusion.Query2[Seated, transform.Transform],
	hier *illusion.Hierarchy,
	assets *poseAssets,
	sk *skins,
	cache *illusion.Local[rideCache],
) {
	c := cache.Get()
	if c.rigs == nil {
		c.rigs, c.posing = map[ecs.Entity]*rideRig{}, map[ecs.Entity]bool{}
	}
	skins := sk.roster.Get().Skins
	bodies.Each(func(e ecs.Entity, st *State, m *render.Model3d, player *render.AnimationPlayer, body *transform.Transform) {
		var seat *Seated
		var root *transform.Transform
		if parent, ok := hier.Parent(e); ok {
			if s, r, ok := seated.Get(parent); ok && s.Anim == Ride && st.Current == Ride && !skins[st.skin].Has(Ride) {
				seat, root = s, r
			}
		}
		if seat == nil {
			if c.posing[e] {
				player.Pose = nil
				delete(c.posing, e)
			}
			return
		}
		model := assets.models.Get().Get(m.Model)
		anims := assets.anims.Get().Get(player.Animations)
		if model == nil || anims == nil {
			return
		}
		bones := model.Skeleton.GetBones()
		rig := c.rigs[e]
		if rig == nil || rig.model != *m {
			rig = makeRideRig(*m, bones)
			c.rigs[e] = rig
		}
		if rig.pelvis < 0 {
			return
		}
		rig.pose = player.SamplePose(&model.Model, anims, rig.pose)
		if len(rig.pose) == 0 {
			return
		}
		// The seat's grips and pegs, in the model's frame.
		matrix := rl.MatrixMultiply(model.Transform, rl.MatrixMultiply(body.Matrix(), root.Matrix()))
		inverse := rl.MatrixInvert(matrix)
		local := func(p rl.Vector3) rl.Vector3 {
			world := rl.Vector3Add(seat.Pose.Translation, rl.Vector3RotateByQuaternion(p, seat.Pose.Rotation))
			return rl.Vector3Transform(world, inverse)
		}
		direction := func(v rl.Vector3) rl.Vector3 {
			world := rl.Vector3RotateByQuaternion(v, seat.Pose.Rotation)
			return rl.Vector3Normalize(rl.Vector3Subtract(rl.Vector3Transform(world, inverse), rl.Vector3Transform(rl.Vector3{}, inverse)))
		}
		up, ahead := direction(rl.Vector3{Y: 1}), direction(rl.Vector3{Z: 1})
		left := rl.Vector3Normalize(rl.Vector3CrossProduct(up, ahead))
		pose := rig.pose
		hips := pose[rig.pelvis].Translation
		reach := func(l rideLimb) rl.Vector3 {
			i := 0
			if l.side < 0 {
				i = 1
			}
			points, fallback, lift := seat.Feet, ridePeg, float32(ankleAbovePeg)
			if l.arm {
				points, fallback, lift = seat.Hands, rideBar, gripBelow
			}
			if len(points) > i {
				return rl.Vector3Add(local(points[i]), rl.Vector3Scale(up, lift))
			}
			f := fallback
			return rl.Vector3Add(hips, rl.Vector3Add(rl.Vector3Scale(left, f.X*l.side), rl.Vector3Add(rl.Vector3Scale(up, f.Y), rl.Vector3Scale(ahead, f.Z))))
		}
		// The wrist sits a little behind the bar it holds.
		wrist := func(l rideLimb) rl.Vector3 { return rl.Vector3Subtract(reach(l), rl.Vector3Scale(ahead, gripAhead)) }

		// Lean as little as gets both hands to their grips. Leaning forward
		// is a turn about the body's right.
		if rig.spine >= 0 {
			pivot := pose[rig.spine].Translation
			right := rl.Vector3Negate(left)
			lean := float32(rideLeanMost)
			for a := float32(rideLeanLeast); a <= rideLeanMost; a += 2 * rl.Deg2rad {
				turn := rl.QuaternionFromAxisAngle(right, -a)
				reachable := true
				for _, l := range rig.limbs {
					if !l.arm {
						continue
					}
					shoulder := rl.Vector3Add(pivot, rl.Vector3RotateByQuaternion(rl.Vector3Subtract(pose[l.upper].Translation, pivot), turn))
					arm := rl.Vector3Distance(pose[l.upper].Translation, pose[l.middle].Translation) +
						rl.Vector3Distance(pose[l.middle].Translation, pose[l.end].Translation)
					if rl.Vector3Distance(shoulder, wrist(l)) > rideReach*arm {
						reachable = false
					}
				}
				if reachable {
					lean = a
					break
				}
			}
			rotateBranch(pose, bones, rig.spine, rl.QuaternionFromAxisAngle(right, -lean))
		}
		for _, l := range rig.limbs {
			target := reach(l)
			if l.arm {
				target = wrist(l)
			}
			upper, middle, end := pose[l.upper].Translation, pose[l.middle].Translation, pose[l.end].Translation
			joint, reached := solveLimb(upper, middle, end, target, nil, 0)
			held := pose[l.end].Rotation
			rotateBranch(pose, bones, l.upper, limbRotation(rl.Vector3Subtract(middle, upper), rl.Vector3Subtract(joint, upper)))
			middle = pose[l.middle].Translation
			rotateBranch(pose, bones, l.middle, limbRotation(rl.Vector3Subtract(pose[l.end].Translation, middle), rl.Vector3Subtract(reached, middle)))
			rotateBranch(pose, bones, l.end, rl.QuaternionMultiply(held, rl.QuaternionInvert(pose[l.end].Rotation)))
			if l.arm {
				grip(pose, bones, l, ahead, up, left)
			}
		}
		player.Pose = pose
		c.posing[e] = true
	})
}

// grip turns a hand about its wrist to take a bar across the body from
// above: fingers ahead and over it, palm down, thumb in toward the middle.
func grip(pose []rl.Transform, bones []rl.BoneInfo, l rideLimb, ahead, up, left rl.Vector3) {
	if l.fingers < 0 || l.index < 0 || l.pinky < 0 {
		return
	}
	wrist := pose[l.end].Translation
	fingers := rl.Vector3Subtract(pose[l.fingers].Translation, wrist)
	across := rl.Vector3Subtract(pose[l.index].Translation, pose[l.pinky].Translation) // toward the thumb
	wantFingers := rl.Vector3Normalize(rl.Vector3Subtract(ahead, rl.Vector3Scale(up, .45)))
	wantAcross := rl.Vector3Scale(left, -l.side) // in toward the middle
	have, want := frame(fingers, across), frame(wantFingers, wantAcross)
	if have == nil || want == nil {
		return
	}
	rotateBranch(pose, bones, l.end, rl.QuaternionMultiply(rl.QuaternionFromMatrix(*want), rl.QuaternionInvert(rl.QuaternionFromMatrix(*have))))
}

// frame is the rotation taking X along along, and Y along across made square
// to it.
func frame(along, across rl.Vector3) *rl.Matrix {
	if rl.Vector3LengthSqr(along) < 1e-8 {
		return nil
	}
	x := rl.Vector3Normalize(along)
	y := rl.Vector3Subtract(across, rl.Vector3Scale(x, rl.Vector3DotProduct(across, x)))
	if rl.Vector3LengthSqr(y) < 1e-8 {
		return nil
	}
	y = rl.Vector3Normalize(y)
	z := rl.Vector3CrossProduct(x, y)
	m := rl.Matrix{
		M0: x.X, M4: y.X, M8: z.X,
		M1: x.Y, M5: y.Y, M9: z.Y,
		M2: x.Z, M6: y.Z, M10: z.Z,
		M15: 1,
	}
	return &m
}
