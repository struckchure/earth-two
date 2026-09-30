package character

import (
	"testing"

	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func TestWear(t *testing.T) {
	r := Roster{Skins: []Skin{
		{Clips: map[Anim]Clip{Idle: {Name: "a-idle"}}, Scale: 1},
		{Clips: map[Anim]Clip{Idle: {Name: "b-idle"}}, Scale: 0.5},
	}}
	st := State{Current: Run}
	var m render.Model3d
	p := render.AnimationPlayer{Paused: true}
	tr := transform.Identity()
	r.Wear(&st, &m, &p, &tr, 1)
	if st.skin != 1 || st.Current != Idle {
		t.Errorf("state = skin %d %v, want skin 1 idle", st.skin, st.Current)
	}
	if p.Clip() != "b-idle" || p.Paused {
		t.Errorf("player: clip %q paused %v, want b-idle playing", p.Clip(), p.Paused)
	}
	if tr.Scale.X != 0.5 || tr.Scale.Y != 0.5 || tr.Scale.Z != 0.5 {
		t.Errorf("scale = %v, want 0.5", tr.Scale)
	}
}

func TestLoopingJumpDoesNotFinish(t *testing.T) {
	var p render.AnimationPlayer
	play(&p, Clip{Name: "jump-loop", Loop: true}, false)
	if p.Clip() != "jump-loop" || p.Finished() {
		t.Fatalf("clip %q finished %v, want jump-loop looping", p.Clip(), p.Finished())
	}
}
