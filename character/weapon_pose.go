package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
)

// Layer the gun's torso over the current gait. The pelvis and legs retain their
// walking/running animation; attachments and clothing receive this same pose.
func layerWeaponPose(pose []rl.Transform, bones []rl.BoneInfo, model *rl.Model, clips *render.Animations, in Intent, scratch []rl.Transform) []rl.Transform {
	if !in.WeaponPose || len(pose) == 0 {
		return scratch
	}
	sample := render.AnimationPlayer{}
	sample.Play("Pistol_Idle_Loop")
	scratch = sample.SamplePose(model, clips, scratch)
	if len(scratch) != len(pose) {
		return scratch
	}
	layerUpperBody(pose, scratch, bones)
	sample.Play("Pistol_Aim_Neutral")
	if in.ShotAge < .24 {
		sample.PlayOnce("Pistol_Shoot")
		sample.Seek(in.ShotAge)
	}
	scratch = sample.SamplePose(model, clips, scratch)
	if len(scratch) != len(pose) {
		return scratch
	}
	layerUpperBodyBlend(pose, scratch, bones, in.WeaponRaise)
	if in.LongGun {
		supportLongGun(pose, bones, in.SupportReach)
	}
	lowerWeaponArms(pose, bones, (1-in.WeaponRaise)*.65)
	for i, bone := range bones {
		if boneName(bone) == "spine_01" {
			rotateBranch(pose, bones, i, rl.QuaternionFromAxisAngle(rl.Vector3{X: 1}, in.WeaponPitch*in.WeaponRaise))
			break
		}
	}
	return scratch
}

func layerUpperBody(pose, upper []rl.Transform, bones []rl.BoneInfo) {
	layerUpperBodyBlend(pose, upper, bones, 1)
}
func layerUpperBodyBlend(pose, upper []rl.Transform, bones []rl.BoneInfo, blend float32) {
	spine, pelvis := -1, -1
	for i, bone := range bones {
		switch boneName(bone) {
		case "spine_01":
			spine = i
		case "pelvis":
			pelvis = i
		}
	}
	if spine < 0 || pelvis < 0 {
		return
	}
	offset := rl.Vector3Subtract(pose[pelvis].Translation, upper[pelvis].Translation)
	for i := range bones {
		for b, steps := i, 0; b >= 0 && b < len(bones) && steps < len(bones); steps++ {
			if b == spine {
				pose[i].Rotation = rl.QuaternionSlerp(pose[i].Rotation, upper[i].Rotation, blend)
				pose[i].Translation = rl.Vector3Lerp(pose[i].Translation, rl.Vector3Add(upper[i].Translation, offset), blend)
				pose[i].Scale = rl.Vector3Lerp(pose[i].Scale, upper[i].Scale, blend)
				break
			}
			b = int(bones[b].Parent)
		}
	}
}

// Bend the firing arm back to the shoulder and place the supporting hand under
// the fore-end, keeping both arm lengths and the animated wrist orientation.
func supportLongGun(pose []rl.Transform, bones []rl.BoneInfo, reach float32) {
	ids := map[string]int{}
	for i, b := range bones {
		ids[boneName(b)] = i
	}
	right, ok := ids["hand_r"]
	if !ok {
		return
	}
	target := rl.Vector3Add(pose[right].Translation, rl.Vector3{Z: -.42, Y: .02})
	placeWeaponHand(pose, bones, ids, "r", target)
	target = rl.Vector3Add(pose[right].Translation, rl.Vector3{Z: reach, Y: .065, X: -.025})
	placeWeaponHand(pose, bones, ids, "l", target)
	hand, okH := ids["hand_l"]
	finger, okF := ids["middle_01_l"]
	index, okI := ids["index_01_l"]
	pinky, okP := ids["pinky_01_l"]
	if okH && okF && okI && okP {
		along := rl.Vector3Subtract(pose[finger].Translation, pose[hand].Translation)
		across := rl.Vector3Subtract(pose[index].Translation, pose[pinky].Translation)
		have, want := frame(along, across), frame(rl.Vector3{X: 1, Y: .25}, rl.Vector3{Z: 1})
		if have != nil && want != nil {
			rotateBranch(pose, bones, hand, rl.QuaternionMultiply(rl.QuaternionFromMatrix(*want), rl.QuaternionInvert(rl.QuaternionFromMatrix(*have))))
		}
	}

}

func placeWeaponHand(pose []rl.Transform, bones []rl.BoneInfo, ids map[string]int, side string, target rl.Vector3) {
	a, okA := ids["upperarm_"+side]
	b, okB := ids["lowerarm_"+side]
	c, okC := ids["hand_"+side]
	if !okA || !okB || !okC {
		return
	}
	start, middle, end := pose[a].Translation, pose[b].Translation, pose[c].Translation
	elbow, wrist := solveLimb(start, middle, end, target, nil, 0)
	rotation := pose[c].Rotation
	rotateBranch(pose, bones, a, limbRotation(rl.Vector3Subtract(middle, start), rl.Vector3Subtract(elbow, start)))
	middle = pose[b].Translation
	rotateBranch(pose, bones, b, limbRotation(rl.Vector3Subtract(pose[c].Translation, middle), rl.Vector3Subtract(wrist, middle)))
	rotateBranch(pose, bones, c, rl.QuaternionMultiply(rotation, rl.QuaternionInvert(pose[c].Rotation)))
}

// Lower both hands about the firing shoulder together, then solve the arms.
// This preserves the two-hand grip while visibly pointing the muzzle down.
func lowerWeaponArms(pose []rl.Transform, bones []rl.BoneInfo, angle float32) {
	if angle < .001 {
		return
	}
	ids := map[string]int{}
	for i, b := range bones {
		ids[boneName(b)] = i
	}
	shoulder, ok := ids["upperarm_r"]
	right, okR := ids["hand_r"]
	left, okL := ids["hand_l"]
	if !ok || !okR || !okL {
		return
	}
	pivot := pose[shoulder].Translation
	rotation := rl.QuaternionFromAxisAngle(rl.Vector3{X: 1}, angle)
	targets := [2]rl.Vector3{}
	orientations := [2]rl.Quaternion{}
	for i, hand := range []int{right, left} {
		targets[i] = rl.Vector3Add(pivot, rl.Vector3RotateByQuaternion(rl.Vector3Subtract(pose[hand].Translation, pivot), rotation))
		orientations[i] = rl.QuaternionMultiply(rotation, pose[hand].Rotation)
	}
	for i, side := range []string{"r", "l"} {
		placeWeaponHand(pose, bones, ids, side, targets[i])
		hand := ids["hand_"+side]
		rotateBranch(pose, bones, hand, rl.QuaternionMultiply(orientations[i], rl.QuaternionInvert(pose[hand].Rotation)))
	}
}
