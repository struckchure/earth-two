package character

import (
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

func contactTestLimb() ([]rl.Transform, []rl.BoneInfo, limbContact) {
	pose := make([]rl.Transform, 4)
	for i, p := range []rl.Vector3{{Y: 1}, {Y: .65, Z: .30}, {Y: .35, Z: .50}, {Y: .30, Z: .64}} {
		pose[i] = rl.Transform{Translation: p, Rotation: rl.QuaternionIdentity(), Scale: rl.Vector3One()}
	}
	bones := []rl.BoneInfo{{Parent: -1}, {Parent: 0}, {Parent: 1}, {Parent: 2}}
	return pose, bones, limbContact{upper: 0, middle: 1, end: 2, radius: .10, tips: []contactPoint{{2, .075}, {3, .10}}}
}

func TestHeldAirPoseStillRespondsToWorldContacts(t *testing.T) {
	models := asset.New[render.Model](nil)
	anims := asset.New[render.Animations](nil)
	bones := []rl.BoneInfo{{Parent: -1}, {Parent: 0}}
	for i, name := range []string{"pelvis", "head"} {
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
	}
	pose := []rl.Transform{{Translation: rl.Vector3{Y: .9}, Rotation: rl.QuaternionIdentity(), Scale: rl.Vector3One()}, {Translation: rl.Vector3{Y: 1.5, Z: .3}, Rotation: rl.QuaternionIdentity(), Scale: rl.Vector3One()}}
	model := models.Add(render.Model{Model: rl.Model{Transform: rl.MatrixIdentity(), Skeleton: rl.ModelSkeleton{BoneCount: 2, Bones: &bones[0], BindPose: &pose[0]}}})
	frames := []rl.ModelAnimPose{&pose[0], &pose[0]}
	clip := rl.ModelAnimation{BoneCount: 2, KeyframeCount: 2, KeyframePoses: &frames[0]}
	copy(clip.Name[:], "held")
	animation := anims.Add(render.Animations{Clips: []rl.ModelAnimation{clip}})
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}).InsertResource(illusion.R(models), illusion.R(anims), illusion.R(&Controls{Enabled: true}))
	defer app.Cleanup()
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: 1.5, Z: .4}, rl.Vector3{X: 4, Y: 3, Z: .1})
		p := render.AnimationPlayer{Animations: animation, Paused: true}
		p.Play("held")
		cmd.Spawn(illusion.C(transform.FromXYZ(0, .9, 0)), illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, Controlled: true}), illusion.C(Traversal{})).WithChild(illusion.C(Body{}), illusion.C(State{}), illusion.C(render.Model3d{Model: model}), illusion.C(p), illusion.C(transform.FromXYZ(0, -.9, 0)))
	}))
	app.AddSystems(illusion.PostUpdate, illusion.Fn8(fitPoseToWorld).Before(transform.Propagate))
	app.Tick(time.Second / 60)
	q := ecs.NewFilter1[render.AnimationPlayer](app.World).Query()
	var player *render.AnimationPlayer
	for q.Next() {
		player = q.Get()
	}
	if len(player.Pose) == 0 || player.Pose[1].Translation.Z > .165 {
		t.Fatal("held head pose penetrates wall")
	}
	before := player.Pose[1].Translation.Z
	roots := ecs.NewFilter2[Traversal, transform.Transform](app.World).Query()
	for roots.Next() {
		_, tr := roots.Get()
		tr.Translation.Z = -.2
	}
	app.Tick(time.Second / 60)
	if len(player.Pose) == 0 || player.Pose[1].Translation.Z <= before {
		t.Fatal("holding an animation froze wall contact correction")
	}
	for range 40 {
		app.Tick(time.Second / 60)
	}
	if len(player.Pose) > 0 {
		t.Fatal("wall clearance survived moving away")
	}
}
func TestLimbContactsPreserveLengthsAndClearSole(t *testing.T) {
	for _, angle := range []float32{0, .7, 1.57, 3.14} {
		pose, bones, limb := contactTestLimb()
		q := rl.QuaternionFromAxisAngle(rl.Vector3{Y: 1}, angle)
		for i := range pose {
			pose[i].Translation = rl.Vector3RotateByQuaternion(pose[i].Translation, q)
			pose[i].Rotation = q
		}
		plane := contactPlane{point: rl.Vector3RotateByQuaternion(rl.Vector3{Z: .34}, q), normal: rl.Vector3RotateByQuaternion(rl.Vector3{Z: -1}, q)}
		a, b := rl.Vector3Distance(pose[0].Translation, pose[1].Translation), rl.Vector3Distance(pose[1].Translation, pose[2].Translation)
		if !fitLimb(pose, bones, &limb, []contactPlane{plane}, false, 1./60) {
			t.Fatal("penetrating pose wasn't corrected")
		}
		if abs(rl.Vector3Distance(pose[0].Translation, pose[1].Translation)-a) > .0001 || abs(rl.Vector3Distance(pose[1].Translation, pose[2].Translation)-b) > .0001 {
			t.Fatal("contact stretched a limb")
		}
		if planeDistance(pose[1].Translation, plane) < limb.radius-.001 {
			t.Fatalf("knee penetrates: %v", pose[1].Translation)
		}
		for _, tip := range limb.tips {
			if planeDistance(pose[tip.bone].Translation, plane) < tip.radius-.001 {
				t.Fatalf("sole penetrates: %+v", pose)
			}
		}
	}
}
func TestLimbCornerAndRelease(t *testing.T) {
	pose, bones, limb := contactTestLimb()
	original := append([]rl.Transform(nil), pose...)
	planes := []contactPlane{{point: rl.Vector3{Z: .35}, normal: rl.Vector3{Z: -1}}, {point: rl.Vector3{X: .16}, normal: rl.Vector3{X: -1}}}
	fitLimb(pose, bones, &limb, planes, false, 1./60)
	for _, p := range planes {
		if planeDistance(pose[1].Translation, p) < limb.radius-.001 {
			t.Fatal("knee folded through corner")
		}
	}
	offset := rl.Vector3Length(limb.offset)
	copy(pose, original)
	fitLimb(pose, bones, &limb, nil, false, 1./60)
	if got := rl.Vector3Length(limb.offset); got <= 0 || got >= offset {
		t.Fatal("contact release should blend back")
	}
	for range 60 {
		copy(pose, original)
		fitLimb(pose, bones, &limb, nil, false, 1./60)
	}
	if rl.Vector3Length(limb.offset) > .001 {
		t.Fatal("contact offset survived leaving the wall")
	}
}
func TestRungFootStaysPlantedWhileKneeFolds(t *testing.T) {
	pose, bones, limb := contactTestLimb()
	pose[1].Translation = rl.Vector3{Y: .65, Z: .5}
	pose[2].Translation = rl.Vector3{Y: .35, Z: .2}
	pose[3].Translation = rl.Vector3{Y: .3, Z: .3}
	foot := pose[2].Translation
	plane := contactPlane{point: rl.Vector3{Z: .4}, normal: rl.Vector3{Z: -1}}
	fitLimb(pose, bones, &limb, []contactPlane{plane}, true, 1./60)
	if rl.Vector3Distance(pose[2].Translation, foot) > .0001 {
		t.Fatal("knee correction moved planted foot")
	}
	if planeDistance(pose[1].Translation, plane) < limb.radius-.001 {
		t.Fatal("knee passed through ladder backing wall")
	}
}

// The authored climb puts the knees between the rails, level with the rungs.
// Only the backing wall may bend them; the ladder itself must not.
func TestLadderGeometryLeavesClimbPoseAlone(t *testing.T) {
	names := []string{"thigh_l", "calf_l", "foot_l", "ball_l", "thigh_r", "calf_r", "foot_r", "ball_r"}
	// Traversal_Ladder at phase 0 on the man, facing +Z.
	points := []rl.Vector3{{X: .115, Y: .975}, {X: .192, Y: .741, Z: .344}, {X: .14, Y: .27, Z: .285}, {X: .183, Y: .201, Z: .411}, {X: -.113, Y: .979}, {X: -.236, Y: .973, Z: .404}, {X: -.14, Y: .52, Z: .285}, {X: -.183, Y: .451, Z: .411}}
	models := asset.New[render.Model](nil)
	anims := asset.New[render.Animations](nil)
	bones := make([]rl.BoneInfo, len(names))
	pose := make([]rl.Transform, len(names))
	for i, name := range names {
		bones[i].Parent = int32(i - 1)
		if i%4 == 0 {
			bones[i].Parent = -1
		}
		for j, c := range name {
			bones[i].Name[j] = int8(c)
		}
		pose[i] = rl.Transform{Translation: points[i], Rotation: rl.QuaternionIdentity(), Scale: rl.Vector3One()}
	}
	model := models.Add(render.Model{Model: rl.Model{Transform: rl.MatrixIdentity(), Skeleton: rl.ModelSkeleton{BoneCount: int32(len(names)), Bones: &bones[0], BindPose: &pose[0]}}})
	frames := []rl.ModelAnimPose{&pose[0], &pose[0]}
	clip := rl.ModelAnimation{BoneCount: int32(len(names)), KeyframeCount: 2, KeyframePoses: &frames[0]}
	copy(clip.Name[:], "held")
	animation := anims.Add(render.Animations{Clips: []rl.ModelAnimation{clip}})
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}).InsertResource(illusion.R(models), illusion.R(anims), illusion.R(&Controls{Enabled: true}))
	defer app.Cleanup()
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		// The traversal course's ladder: rails and rungs 37 cm ahead, wall at 65 cm.
		staticBox(cmd, rl.Vector3{Y: 1.4, Z: 1.65}, rl.Vector3{X: 3, Y: 2.8, Z: 2})
		for _, x := range []float32{-.32, .32} {
			staticBox(cmd, rl.Vector3{X: x, Y: 1.45, Z: .37}, rl.Vector3{X: .07, Y: 2.9, Z: .07})
		}
		for y := float32(.2); y < 2.9; y += .25 {
			staticBox(cmd, rl.Vector3{Y: y, Z: .37}, rl.Vector3{X: .71, Y: .045, Z: .06})
		}
		cmd.Spawn(illusion.C(Ladder{Top: rl.Vector3{Y: 2.84}, Facing: rl.Vector3{Z: 1}, Width: .8, RungSpacing: .25}))
		p := render.AnimationPlayer{Animations: animation, Paused: true}
		p.Play("held")
		cmd.Spawn(illusion.C(transform.FromXYZ(0, .92, 0)), illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, Controlled: true}), illusion.C(Traversal{})).WithChild(illusion.C(Body{}), illusion.C(State{}), illusion.C(render.Model3d{Model: model}), illusion.C(p), illusion.C(transform.FromXYZ(0, -.9, 0)))
	}))
	app.AddSystems(illusion.PostUpdate, illusion.Fn8(fitPoseToWorld).Before(transform.Propagate))
	app.Tick(time.Second / 60)
	var ladder ecs.Entity
	for q := ecs.NewFilter1[Ladder](app.World).Query(); q.Next(); {
		ladder = q.Entity()
	}
	// The exit starts from the same pose, still on the rungs.
	for _, mode := range []Anim{LadderClimb, LadderExit} {
		for q := ecs.NewFilter1[Traversal](app.World).Query(); q.Next(); {
			s := q.Get()
			s.Mode, s.Ladder = mode, ladder
		}
		app.Tick(time.Second / 60)
		for q := ecs.NewFilter1[render.AnimationPlayer](app.World).Query(); q.Next(); {
			if p := q.Get(); len(p.Pose) > 0 {
				t.Fatalf("%v: ladder splayed the knees: %v %v", mode, p.Pose[1].Translation, p.Pose[5].Translation)
			}
		}
	}
}
