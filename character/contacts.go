package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// The capsule moves the character; these contacts keep the animated limbs
// outside nearby static walls without turning the skeleton into a ragdoll.
// Solve the blended pose, so transitions and clothing obey the same contacts.
type poseAssets struct {
	models   illusion.Res[asset.Assets[render.Model]]
	anims    illusion.Res[asset.Assets[render.Animations]]
	controls illusion.Res[Controls]
	settings illusion.Res[physics.Settings]
}

func (a *poseAssets) InitParam(w *ecs.World) {
	a.models.InitParam(w)
	a.anims.InitParam(w)
	a.controls.InitParam(w)
	a.settings.InitParam(w)
}

type limbContact struct {
	upper, middle, end int
	tips               []contactPoint
	radius             float32
	leg                bool
	offset             rl.Vector3
}
type contactPoint struct {
	bone   int
	radius float32
}
type contactRig struct {
	model      asset.Handle[render.Model]
	limbs      []limbContact
	pose       []rl.Transform
	torso      []contactPoint
	offset     rl.Vector3 // world-space posture clearance, released smoothly
	position   rl.Vector3
	positioned bool
}
type contactCache struct{ rigs map[ecs.Entity]*contactRig }
type contactPlane struct{ point, normal rl.Vector3 }

// ladderDepth separates a ladder's own rails and rungs, 37 cm ahead of the
// climb axis, from the backing wall behind them (65 cm or more).
const ladderDepth = .5

func makeContactRig(model asset.Handle[render.Model], bones []rl.BoneInfo) *contactRig {
	r := &contactRig{model: model}
	indices := map[string]int{}
	for i, b := range bones {
		indices[boneName(b)] = i
	}
	for _, name := range []string{"pelvis", "spine_02", "spine_03", "neck_01", "head"} {
		if index, ok := indices[name]; ok {
			radius := float32(.21)
			if name == "head" {
				radius = .18
			}
			if name == "neck_01" {
				radius = .12
			}
			r.torso = append(r.torso, contactPoint{index, radius})
		}
	}
	for _, side := range []string{"l", "r"} {
		for _, leg := range []bool{false, true} {
			names := []string{"upperarm_" + side, "lowerarm_" + side, "hand_" + side}
			radius := float32(.085)
			points := map[string]float32{"hand_" + side: .055, "middle_01_" + side: .06, "middle_03_" + side: .04, "index_03_" + side: .04, "thumb_03_" + side: .035}
			if leg {
				names = []string{"thigh_" + side, "calf_" + side, "foot_" + side}
				radius = .14 // includes the thigh/knee surface and fitted trousers
				points = map[string]float32{"foot_" + side: .085, "ball_" + side: .12}
			}
			upper, a := indices[names[0]]
			middle, b := indices[names[1]]
			end, c := indices[names[2]]
			if !a || !b || !c {
				continue
			}
			limb := limbContact{upper: upper, middle: middle, end: end, radius: radius, leg: leg}
			// Skeleton order is stable, unlike map iteration; contacts must be too.
			for i, bone := range bones {
				if size, ok := points[boneName(bone)]; ok {
					limb.tips = append(limb.tips, contactPoint{i, size})
				}
			}
			r.limbs = append(r.limbs, limb)
		}
	}
	return r
}

func fitPoseToWorld(
	bodies *illusion.Query4Where[State, render.Model3d, render.AnimationPlayer, transform.Transform, illusion.With[Body]],
	roots *illusion.Query3[transform.Transform, physics.CharacterController, Traversal],
	hier *illusion.Hierarchy, assets *poseAssets, world *physics.Physics,
	cache *illusion.Local[contactCache], clock *illusion.Res[illusion.Time],
	ladders *illusion.Query1[Ladder],
) {
	c := cache.Get()
	if c.rigs == nil {
		c.rigs = map[ecs.Entity]*contactRig{}
	}
	bodies.Each(func(e ecs.Entity, state *State, m *render.Model3d, player *render.AnimationPlayer, body *transform.Transform) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		root, _, traversal, ok := roots.Get(parent)
		if !ok {
			// Not standing (seated, say): it fits nothing to the world, so it
			// takes back the pose it last fitted, or the body would stay
			// frozen in it. A pose someone else set (ride's) is theirs.
			if rig := c.rigs[e]; rig != nil && len(player.Pose) > 0 && len(rig.pose) > 0 && &player.Pose[0] == &rig.pose[0] {
				player.Pose = nil
			}
			return
		}
		model := assets.models.Get().Get(m.Model)
		anims := assets.anims.Get().Get(player.Animations)
		if model == nil || anims == nil {
			return
		}
		rig := c.rigs[e]
		if rig == nil || rig.model != m.Model {
			rig = makeContactRig(m.Model, model.Skeleton.GetBones())
			c.rigs[e] = rig
		}
		if rig.positioned && rl.Vector3DistanceSqr(root.Translation, rig.position) > 1 {
			rig.offset = rl.Vector3{}
			for i := range rig.limbs {
				rig.limbs[i].offset = rl.Vector3{}
			}
		}
		rig.position, rig.positioned = root.Translation, true
		if (!assets.controls.Get().Enabled || assets.settings.Get().Paused) && len(rig.pose) > 0 {
			return
		}
		rig.pose = player.SamplePose(&model.Model, anims, rig.pose)
		player.Pose = nil
		if len(rig.pose) == 0 {
			return
		}
		corrected := false
		matrix := rl.MatrixMultiply(model.Transform, rl.MatrixMultiply(body.Matrix(), root.Matrix()))
		inverse := rl.MatrixInvert(matrix)
		origin := rl.Vector3Transform(rl.Vector3{}, matrix)
		scale := rl.Vector3Length(rl.Vector3{X: matrix.M0, Y: matrix.M1, Z: matrix.M2})
		climbing := traversal.Mode == LadderClimb || traversal.Mode == LadderEnter
		// The climb pose is authored against the rails and rungs; a
		// knee between them is not a knee through a wall.
		solid := func(hit physics.RayHit) bool {
			return world.StaticSurface(hit.Entity) && abs(hit.Normal.Y) <= .2
		}
		// The climb off is authored against the ladder and its landing from
		// start to finish: correcting it only makes it twitch.
		if traversal.Mode == LadderExit {
			rig.offset = rl.Vector3{}
			for i := range rig.limbs {
				rig.limbs[i].offset = rl.Vector3{}
			}
			return
		}
		if climbing {
			if ladder, ok := ladders.Get(traversal.Ladder); ok {
				bottom, facing := ladder.Bottom, direction(horizontal(ladder.Facing))
				solid = func(hit physics.RayHit) bool {
					return world.StaticSurface(hit.Entity) && abs(hit.Normal.Y) <= .2 && rl.Vector3DotProduct(rl.Vector3Subtract(hit.Point, bottom), facing) >= ladderDepth
				}
			}
		}
		if climbing {
			rig.offset = rl.Vector3{}
		} else {
			corrected = fitTorso(rig, world, parent, solid, matrix, inverse, origin, scale, clock.Get().DeltaSecs())
		}
		for i := range rig.limbs {
			limb := &rig.limbs[i]
			// Authored rail grips and rung feet remain exact while attached. Knees
			// still fold away from the ladder's backing wall.
			if climbing && !limb.leg {
				limb.offset = rl.Vector3{}
				continue
			}
			probes := append([]contactPoint{{limb.middle, limb.radius}}, limb.tips...)
			var planes []contactPlane
			for _, probe := range probes {
				point := rl.Vector3Transform(rig.pose[probe.bone].Translation, matrix)
				from := origin
				from.Y = point.Y
				delta := horizontal(rl.Vector3Subtract(point, from))
				length := rl.Vector3Length(delta)
				if length < .01 {
					continue
				}
				dir := rl.Vector3Scale(delta, 1/length)
				// Sampling above/below a rung finds the backing wall instead of
				// treating the intended foot contact as a reason to move off it.
				offsets := []float32{0}
				if climbing {
					offsets = []float32{-.10, .10}
				}
				for _, offset := range offsets {
					start := from
					start.Y += offset
					hit, found := world.CastRayExcluding(start, dir, length+4*probe.radius*scale+.04, parent)
					if !found || !solid(hit) {
						continue
					}
					p := contactPlane{point: rl.Vector3Transform(hit.Point, inverse)}
					normalEnd := rl.Vector3Transform(rl.Vector3Add(hit.Point, hit.Normal), inverse)
					p.normal = rl.Vector3Normalize(rl.Vector3Subtract(normalEnd, p.point))
					duplicate := false
					for _, other := range planes {
						if rl.Vector3DotProduct(p.normal, other.normal) > .995 && abs(rl.Vector3DotProduct(rl.Vector3Subtract(p.point, other.point), other.normal)) < .02 {
							duplicate = true
							break
						}
					}
					if !duplicate {
						planes = append(planes, p)
					}
				}
			}
			corrected = fitLimb(rig.pose, model.Skeleton.GetBones(), limb, planes, climbing, clock.Get().DeltaSecs()) || corrected
		}
		if corrected {
			player.Pose = rig.pose
		}
	})
	// Bodies can be replaced by wardrobe/respawn. Do not retain old buffers.
	for e := range c.rigs {
		if _, _, _, _, ok := bodies.Get(e); !ok {
			delete(c.rigs, e)
		}
	}
}

// A leaning jump can put the head/chest beyond the controller. Reserve that
// space for the entire posture before bending individual limbs; this keeps
// the spine intact and releases the small offset smoothly after leaving.
func fitTorso(rig *contactRig, world *physics.Physics, root ecs.Entity, solid func(physics.RayHit) bool, matrix, inverse rl.Matrix, origin rl.Vector3, scale, dt float32) bool {
	offset := rl.Vector3Scale(rig.offset, float32(math.Exp(float64(-18*dt))))
	for range 2 {
		for _, probe := range rig.torso {
			point := rl.Vector3Add(rl.Vector3Transform(rig.pose[probe.bone].Translation, matrix), offset)
			from := origin
			from.Y = point.Y
			delta := horizontal(rl.Vector3Subtract(point, from))
			length := rl.Vector3Length(delta)
			if length < .005 {
				continue
			}
			dir := rl.Vector3Scale(delta, 1/length)
			radius := probe.radius * scale
			for _, height := range []float32{0, -radius * .6, radius * .6} {
				start := from
				start.Y += height
				hit, ok := world.CastRayExcluding(start, dir, length+4*radius, root)
				if !ok || !solid(hit) {
					continue
				}
				depth := radius + .015 - rl.Vector3DotProduct(rl.Vector3Subtract(point, hit.Point), hit.Normal)
				if depth > 0 {
					push := rl.Vector3Scale(hit.Normal, depth)
					offset = rl.Vector3Add(offset, push)
					point = rl.Vector3Add(point, push)
				}
			}
		}
	}
	if rl.Vector3LengthSqr(offset) < .000001 {
		rig.offset = rl.Vector3{}
		return false
	}
	rig.offset = offset
	local := rl.Vector3Subtract(rl.Vector3Transform(rl.Vector3Add(origin, offset), inverse), rl.Vector3Transform(origin, inverse))
	for i := range rig.pose {
		rig.pose[i].Translation = rl.Vector3Add(rig.pose[i].Translation, local)
	}
	return true
}

func planeDistance(point rl.Vector3, plane contactPlane) float32 {
	return rl.Vector3DotProduct(rl.Vector3Subtract(point, plane.point), plane.normal)
}

func fitLimb(pose []rl.Transform, bones []rl.BoneInfo, limb *limbContact, planes []contactPlane, keepFoot bool, dt float32) bool {
	original := pose[limb.end].Translation
	target := rl.Vector3Add(original, rl.Vector3Scale(limb.offset, float32(math.Exp(float64(-18*dt)))))
	if keepFoot {
		target = original
	}
	if !keepFoot {
		// Project the entire palm/sole envelope, not only the wrist or ankle.
		for range 4 {
			for _, plane := range planes {
				shift := rl.Vector3Subtract(target, original)
				depth := float32(0)
				for _, tip := range limb.tips {
					point := rl.Vector3Add(pose[tip.bone].Translation, shift)
					depth = max(depth, tip.radius+.008-planeDistance(point, plane))
				}
				if depth > 0 {
					target = rl.Vector3Add(target, rl.Vector3Scale(plane.normal, depth))
				}
			}
		}
	}
	upper, middle := pose[limb.upper].Translation, pose[limb.middle].Translation
	blocked := rl.Vector3DistanceSqr(target, original) > .000001
	for _, plane := range planes {
		blocked = blocked || planeDistance(middle, plane) < limb.radius+.008
	}
	if !blocked {
		limb.offset = rl.Vector3{}
		return false
	}
	elbow, end := solveLimb(upper, middle, original, target, planes, limb.radius+.008)
	orientation := pose[limb.end].Rotation
	rotateBranch(pose, bones, limb.upper, limbRotation(rl.Vector3Subtract(middle, upper), rl.Vector3Subtract(elbow, upper)))
	middle = pose[limb.middle].Translation
	rotateBranch(pose, bones, limb.middle, limbRotation(rl.Vector3Subtract(pose[limb.end].Translation, middle), rl.Vector3Subtract(end, middle)))
	rotateBranch(pose, bones, limb.end, rl.QuaternionMultiply(orientation, rl.QuaternionInvert(pose[limb.end].Rotation)))
	limb.offset = rl.Vector3Subtract(pose[limb.end].Translation, original)
	return true
}

// An analytic two-bone solve preserves lengths. Search its elbow/knee circle
// for a bend outside the walls, preferring the authored pose over a pole flip.
func solveLimb(start, middle, end, target rl.Vector3, planes []contactPlane, radius float32) (rl.Vector3, rl.Vector3) {
	a, b := rl.Vector3Distance(start, middle), rl.Vector3Distance(middle, end)
	delta := rl.Vector3Subtract(target, start)
	distance := rl.Vector3Length(delta)
	if a < .001 || b < .001 || distance < .001 {
		return middle, end
	}
	dir := rl.Vector3Scale(delta, 1/distance)
	distance = clamp(distance, abs(a-b)+.0001, a+b-.0001)
	target = rl.Vector3Add(start, rl.Vector3Scale(dir, distance))
	along := (a*a - b*b + distance*distance) / (2 * distance)
	height := float32(math.Sqrt(float64(max(0, a*a-along*along))))
	center := rl.Vector3Add(start, rl.Vector3Scale(dir, along))
	bend := rl.Vector3Subtract(middle, center)
	bend = rl.Vector3Subtract(bend, rl.Vector3Scale(dir, rl.Vector3DotProduct(bend, dir)))
	if rl.Vector3LengthSqr(bend) < .000001 {
		bend = rl.Vector3CrossProduct(dir, rl.Vector3{Z: 1})
		if rl.Vector3LengthSqr(bend) < .000001 {
			bend = rl.Vector3CrossProduct(dir, rl.Vector3{X: 1})
		}
	}
	bend = rl.Vector3Normalize(bend)
	across := rl.Vector3CrossProduct(dir, bend)
	best := middle
	score := float32(math.MaxFloat32)
	for i := range 48 {
		angle := float64(i) * 2 * math.Pi / 48
		radial := rl.Vector3Add(rl.Vector3Scale(bend, float32(math.Cos(angle))), rl.Vector3Scale(across, float32(math.Sin(angle))))
		candidate := rl.Vector3Add(center, rl.Vector3Scale(radial, height))
		cost := rl.Vector3DistanceSqr(candidate, middle)
		for _, plane := range planes {
			depth := max(0, radius-planeDistance(candidate, plane))
			cost += depth * depth * 10000
		}
		if cost < score {
			best, score = candidate, cost
		}
	}
	return best, target
}

func rotateBranch(pose []rl.Transform, bones []rl.BoneInfo, root int, rotation rl.Quaternion) {
	rotation = rl.QuaternionNormalize(rotation)
	pivot := pose[root].Translation
	for i := range pose {
		current := i
		for current != root && current >= 0 && current < len(bones) {
			parent := int(bones[current].Parent)
			if parent == current {
				current = -1
				break
			}
			current = parent
		}
		if current != root {
			continue
		}
		pose[i].Translation = rl.Vector3Add(pivot, rl.Vector3RotateByQuaternion(rl.Vector3Subtract(pose[i].Translation, pivot), rotation))
		pose[i].Rotation = rl.QuaternionMultiply(rotation, pose[i].Rotation)
	}
}

func limbRotation(from, to rl.Vector3) rl.Quaternion {
	if rl.Vector3LengthSqr(from) < .000001 || rl.Vector3LengthSqr(to) < .000001 {
		return rl.QuaternionIdentity()
	}
	from, to = rl.Vector3Normalize(from), rl.Vector3Normalize(to)
	if rl.Vector3DotProduct(from, to) < -.9999 {
		axis := rl.Vector3CrossProduct(from, rl.Vector3{Y: 1})
		if rl.Vector3LengthSqr(axis) < .000001 {
			axis = rl.Vector3CrossProduct(from, rl.Vector3{X: 1})
		}
		return rl.QuaternionFromAxisAngle(rl.Vector3Normalize(axis), math.Pi)
	}
	return rl.QuaternionFromVector3ToVector3(from, to)
}
