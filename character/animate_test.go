package character

import (
	"testing"

	"github.com/struckchure/illusion/render"
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
		{"moving cuts an action short", motion{Grounded: true, Speed: 2}, Interact, true, Walk},
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
