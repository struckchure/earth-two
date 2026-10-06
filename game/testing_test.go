package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/render"
)

func TestShadowSwitch(t *testing.T) {
	app := illusion.New()
	shadows := &render.Shadows{Size: shadowSize, Range: shadowRange}
	app.InsertResource(illusion.R(shadows))
	s := shadowSwitch()
	if s.Key != rl.KeyF7 || s.Get(app.World) != 0 {
		t.Fatal("shadow switch should start at Full on F7")
	}
	for _, tt := range []struct {
		mode int
		size int32
	}{{1, shadowSize / 2}, {2, 0}, {0, shadowSize}} {
		s.Set(app.World, tt.mode)
		if shadows.Size != tt.size || s.Get(app.World) != tt.mode || shadows.Range != shadowRange {
			t.Fatalf("shadow mode %d: %+v", tt.mode, shadows)
		}
	}
}

// Switches get the free function keys in turn, and each press steps one
// through its settings and back round: the weather from its schedule to
// clear, dusty, a storm, and back to its schedule.
func TestTestSwitches(t *testing.T) {
	app := illusion.New()
	keys := input.NewButtonInput[input.Key]()
	app.InsertResource(illusion.R(keys), illusion.R(newWeather()))
	addTestSwitch(app, weatherSwitch())
	other := 0
	addTestSwitch(app, testSwitch{Name: "Other", Settings: []string{"a", "b"},
		Get: func(*ecs.World) int { return other }, Set: func(_ *ecs.World, i int) { other = i }})
	app.AddSystems(illusion.Update, illusion.Fn3(flipSwitches))
	sw := ecs.GetResource[testSwitches](app.World)
	if sw.list[0].Key != rl.KeyF5 || sw.list[1].Key != rl.KeyF6 {
		t.Fatalf("switches should get F5 and F6, got %v and %v", sw.list[0].Key, sw.list[1].Key)
	}
	press := func(k input.Key) {
		keys.Press(k)
		app.Tick(time.Second / 60)
		keys.Release(k)
		keys.Clear()
	}
	press(rl.KeyF1)
	if !sw.shown {
		t.Fatal("F1 should show the panel")
	}
	wt := ecs.GetResource[weather](app.World)
	for _, want := range []float32{0, .45, 1, -1} {
		press(rl.KeyF5)
		if wt.forced != want {
			t.Fatalf("F5 should step the weather on to %v, it's %v", want, wt.forced)
		}
	}
	press(rl.KeyF6)
	if other != 1 {
		t.Fatal("F6 should step the other switch")
	}
}
