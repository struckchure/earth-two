package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
)

// Redirect the existing footfall cycle in character-local space. Foot lift and
// timing stay authored; the two-bone solve keeps knees bending and feet facing
// the aim direction instead of rotating the whole character sideways/backward.
func weaponGait(pose []rl.Transform, bones []rl.BoneInfo, direction rl.Vector3) {
	directionalGait(pose, bones, direction, false)
}

// Keep the authored low pelvis and foot lift, with short lateral steps that
// stay on their own side of the body instead of crossing the bent knees.
func crouchGait(pose []rl.Transform, bones []rl.BoneInfo, direction rl.Vector3) {
	directionalGait(pose, bones, direction, true)
}

func directionalGait(pose []rl.Transform, bones []rl.BoneInfo, direction rl.Vector3, crouched bool) {
	ids := map[string]int{}
	for i, b := range bones {
		ids[boneName(b)] = i
	}
	for _, side := range []string{"l", "r"} {
		a, okA := ids["thigh_"+side]
		b, okB := ids["calf_"+side]
		c, okC := ids["foot_"+side]
		if !okA || !okB || !okC {
			continue
		}
		hip, knee, foot := pose[a].Translation, pose[b].Translation, pose[c].Translation
		stride := foot.Z - hip.Z
		target := foot
		target.X += stride * direction.X
		target.Z = hip.Z + stride*direction.Z
		if crouched {
			lateral := min(float32(1), float32(math.Abs(float64(direction.X))))
			target.X = hip.X + stride*direction.X*.65
			if side == "l" {
				target.X = max(target.X, .10)
			} else {
				target.X = min(target.X, -.10)
			}
			// Preserve some stagger for a stable firing stance while side-stepping.
			target.Z = hip.Z + stride*(direction.Z+.25*lateral)
		}
		newKnee, newFoot := solveLimb(hip, knee, foot, target, nil, 0)
		rotation := pose[c].Rotation
		rotateBranch(pose, bones, a, limbRotation(rl.Vector3Subtract(knee, hip), rl.Vector3Subtract(newKnee, hip)))
		knee = pose[b].Translation
		rotateBranch(pose, bones, b, limbRotation(rl.Vector3Subtract(pose[c].Translation, knee), rl.Vector3Subtract(newFoot, knee)))
		rotateBranch(pose, bones, c, rl.QuaternionMultiply(rotation, rl.QuaternionInvert(pose[c].Rotation)))
	}
}

// Blend the capture on a persistent distance-driven clock, independent of
// walk/run clip restarts and idle transitions.
func layerStrafeClip(pose []rl.Transform, bones []rl.BoneInfo, model *rl.Model, clips *render.Animations, phase, lateral float32, scratch []rl.Transform) []rl.Transform {
	weight := min(float32(1), float32(math.Abs(float64(lateral))))
	if weight < .001 {
		return scratch
	}
	name := "Weapon_StrafeLeft"
	if lateral < 0 {
		name = "Weapon_StrafeRight"
	}
	source, ok := clips.Clip(name)
	if !ok {
		return scratch
	}
	// Mirroring swaps feet; shift half a cycle to preserve the support foot.
	if lateral < 0 {
		phase += .5
	}
	phase -= float32(math.Floor(float64(phase)))
	sample := render.AnimationPlayer{}
	sample.Play(name)
	sample.Seek(phase * clips.Duration(source))
	scratch = sample.SamplePose(model, clips, scratch)
	if len(scratch) != len(pose) {
		return scratch
	}
	for i := range bones {
		if lowerBodyBone(bones, i) {
			pose[i].Translation = rl.Vector3Lerp(pose[i].Translation, scratch[i].Translation, weight)
			pose[i].Rotation = rl.QuaternionSlerp(pose[i].Rotation, scratch[i].Rotation, weight)
		}
	}
	return scratch
}

// The capture travels about 1.28m per full two-foot cycle. Keeping its phase
// independent of the forward animation prevents a reset when walk becomes run.
func advanceGaitPhase(phase, speed, dt float32) float32 {
	phase += max(float32(0), speed) * max(float32(0), dt) / 1.28
	return phase - float32(math.Floor(float64(phase)))
}
func lowerBodyBone(bones []rl.BoneInfo, i int) bool {
	for b, steps := i, 0; b >= 0 && b < len(bones) && steps < len(bones); steps++ {
		name := boneName(bones[b])
		if name == "spine_01" {
			return false
		}
		if name == "pelvis" {
			return true
		}
		b = int(bones[b].Parent)
	}
	return false
}

// Smooth the final lower-body pose, including entry/exit and direction changes.
// Blending only the source weight missed idle transitions and reset foot poses.
func smoothWeaponGait(pose, previous []rl.Transform, bones []rl.BoneInfo, dt float32) []rl.Transform {
	if len(previous) != len(pose) {
		return append(previous[:0], pose...)
	}
	k := float32(1 - math.Exp(float64(-20*max(float32(0), dt))))
	for i := range pose {
		if lowerBodyBone(bones, i) {
			pose[i].Translation = rl.Vector3Lerp(previous[i].Translation, pose[i].Translation, k)
			pose[i].Rotation = rl.QuaternionSlerp(previous[i].Rotation, pose[i].Rotation, k)
		}
		previous[i] = pose[i]
	}
	return previous
}
