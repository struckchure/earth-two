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

func TestPickAnim(t *testing.T) {
	c := Default() // walk 1.6, run 4.6: runs above 3.1
	tests := []struct {
		name    string
		m       motion
		current Anim
		acting  bool
		want    Anim
	}{
		{"standing", motion{Grounded: true}, Idle, false, Idle},
		{"barely drifting", motion{Grounded: true, Speed: 0.2}, Idle, false, Idle},
		{"walking", motion{Grounded: true, Speed: 1.6}, Idle, false, Walk},
		{"jogging stays a walk", motion{Grounded: true, Speed: 3}, Walk, false, Walk},
		{"running", motion{Grounded: true, Speed: 5}, Walk, false, Run},
		{"jumping from standing", motion{}, Idle, false, Jump},
		{"jumping on the move", motion{Speed: 5}, Run, false, RunJump},
		{"a standing jump stays one when steered", motion{Speed: 5}, Jump, false, Jump},
		{"a running jump stays one when stopped", motion{}, RunJump, false, RunJump},
		{"landing still", motion{Grounded: true}, Jump, false, Idle},
		{"landing on the move", motion{Grounded: true, Speed: 5}, RunJump, false, Run},
		{"acting", motion{Grounded: true}, Punch, true, Punch},
		{"action done", motion{Grounded: true}, Punch, false, Idle},
		{"a shove doesn't cut an action short", motion{Grounded: true, Speed: 2}, Interact, true, Interact},
		{"moving once it's done", motion{Grounded: true, Speed: 2}, Interact, false, Walk},
		{"walking on out of an action", motion{Grounded: true, Resuming: true}, Punch, false, Walk},
		{"running on out of an action starts with a walk", motion{Grounded: true, Resuming: true}, PickUp, false, Walk},
		{"still walking while speeding up", motion{Grounded: true, Speed: 0.1, Resuming: true}, Walk, false, Walk},
		{"a finished action with nowhere to go", motion{Grounded: true}, Punch, false, Idle},
		{"stepping round turning about", motion{Grounded: true, Speed: 0.1, Pivoting: true}, Walk, false, Walk},
		{"stepping round from standing", motion{Grounded: true, Pivoting: true}, Idle, false, Walk},
		{"falling cuts an action short", motion{}, PickUp, true, Jump},
	}
	for _, tt := range tests {
		if got := pickAnim(tt.m, c, tt.current, tt.acting); got != tt.want {
			t.Errorf("%s: pickAnim = %v, want %v", tt.name, got, tt.want)
		}
	}
}

func TestAnimKinds(t *testing.T) {
	for a := Idle; a <= PickUp; a++ {
		if a.Airborne() && a.OneShot() {
			t.Errorf("%v is both airborne and a one-shot", a)
		}
	}
	for _, a := range []Anim{Interact, Punch, PickUp} {
		if !a.OneShot() {
			t.Errorf("%v isn't a one-shot", a)
		}
	}
	for _, a := range []Anim{Jump, RunJump} {
		if !a.Airborne() {
			t.Errorf("%v isn't airborne", a)
		}
	}
}

func TestSkinClipFallbacks(t *testing.T) {
	s := Skin{Clips: map[Anim]Clip{
		Idle: {Name: "idle"},
		Jump: {Name: "jump", Start: 0.3},
	}}
	if c := s.clip(RunJump); c.Name != "jump" || c.Start != 0.3 {
		t.Errorf("RunJump without a clip plays %+v, want the jump", c)
	}
	if c := s.clip(Walk); c.Name != "idle" {
		t.Errorf("Walk without a clip plays %+v, want idle", c)
	}
	if s.Has(Punch) {
		t.Error("Has(Punch) with no punch clip")
	}
}

func TestAirSpeed(t *testing.T) {
	// A 0.4s take-off-to-landing stretched over a 4 m/s jump in 10 m/s²
	// gravity, which lasts 0.8s: half speed.
	if got := airSpeed(Clip{Start: 0.3, Land: 0.7}, 4, 10); got < 0.4999 || got > 0.5001 {
		t.Errorf("airSpeed = %v, want 0.5", got)
	}
	if got := airSpeed(Clip{Start: 0.3}, 4, 10); got != 1 {
		t.Errorf("airSpeed without Land = %v, want 1", got)
	}
	if got := airSpeed(Clip{Start: 0, Land: 0.01}, 4, 10); got != 0.25 {
		t.Errorf("airSpeed for a blink of a jump = %v, want the 0.25 floor", got)
	}
}

func TestPlayStartsAtStart(t *testing.T) {
	var p render.AnimationPlayer
	p.Paused = true // left over from a jump held at Land
	play(&p, Clip{Name: "run", Start: 0.3, Hold: true}, true)
	if p.Clip() != "run" || p.Paused || p.Time() != 0.3 {
		t.Fatalf("clip %q paused %v at %v, want run playing from 0.3", p.Clip(), p.Paused, p.Time())
	}
}

func TestWallKickAnimationSelectionAndRecovery(t *testing.T) {
	store := asset.New[render.Animations](nil)
	clips := []rl.ModelAnimation{{KeyframeCount: 37}, {KeyframeCount: 61}, {KeyframeCount: 61}, {KeyframeCount: 37}, {KeyframeCount: 28}}
	for i, name := range []string{"Traversal_WallKick", "idle", "jump", "Traversal_WallKickFall", "Traversal_WallLand"} {
		copy(clips[i].Name[:], name)
	}
	handle := store.Add(render.Animations{Clips: clips})
	controls := &Controls{Enabled: true}
	app := illusion.New().AddPlugins(transform.Plugin{}).InsertResource(illusion.R(store), illusion.R(controls), illusion.R(&physics.Settings{}), illusion.R(&Roster{Skins: []Skin{{Anims: handle, Clips: map[Anim]Clip{Idle: {Name: "idle"}, Jump: {Name: "jump"}, WallKick: {Name: "Traversal_WallKick"}, WallFall: {Name: "Traversal_WallKickFall"}, WallLand: {Name: "Traversal_WallLand"}}}}}))
	defer app.Cleanup()
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(Default()), illusion.C(Intent{}), illusion.C(physics.CharacterController{}), illusion.C(Traversal{Kick: wallKickTime, WallNormal: rl.Vector3{Z: 1}}), illusion.C(MotionSamples{}), illusion.C(transform.Identity())).WithChild(illusion.C(Body{}), illusion.C(State{Current: Jump}), illusion.C(render.AnimationPlayer{Animations: handle}), illusion.C(transform.Identity()))
	}))
	app.AddSystems(illusion.Update, illusion.Chain(illusion.Fn5(face), illusion.Fn8(animate)))
	app.Tick(time.Second / 60)
	q := ecs.NewFilter3[State, render.AnimationPlayer, transform.Transform](app.World).Query()
	var state *State
	var player *render.AnimationPlayer
	var body *transform.Transform
	for q.Next() {
		state, player, body = q.Get()
	}
	roots := ecs.NewFilter2[Traversal, physics.CharacterController](app.World).Query()
	var traversal *Traversal
	var cc *physics.CharacterController
	for roots.Next() {
		traversal, cc = roots.Get()
	}
	if state.Current != WallKick || player.Clip() != "Traversal_WallKick" || !player.ManualTime {
		t.Fatalf("kick did not select dedicated clip: %+v", state)
	}
	traversal.Kick = wallKickTime / 2
	app.Tick(time.Second / 60)
	if player.Time() < .25 || player.Time() > .31 {
		t.Fatalf("kick progress=%v", player.Time())
	}
	controls.Enabled = false
	stamp, rotation := player.Time(), body.Rotation
	app.Tick(time.Second / 30)
	if player.Time() != stamp || body.Rotation != rotation || !player.Paused {
		t.Fatal("pause advanced kick pose/facing")
	}
	controls.Enabled = true
	traversal.Kick = wallKickTime // another wall in the same airtime
	app.Tick(time.Second / 60)
	if player.Time() > .03 || player.Paused {
		t.Fatal("chained bounce did not restart pose")
	}
	traversal.Kick = 0 // the kick is over, still in the air
	app.Tick(time.Second / 60)
	if state.Current != WallFall || player.Clip() != "Traversal_WallKickFall" || player.ManualTime || player.Paused {
		t.Fatalf("kick did not go on into its fall: %v %q", state.Current, player.Clip())
	}
	cc.Grounded = true
	app.Tick(time.Second / 60)
	if state.Current != WallLand || player.Clip() != "Traversal_WallLand" || player.ManualTime {
		t.Fatalf("landing did not play: %v %q", state.Current, player.Clip())
	}
}
