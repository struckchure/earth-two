package game

import (
	"fmt"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/window"
)

// The test plugin: tools for trying the game out without playing up to
// what's being tried. F1 shows its panel, which lists them:
//
//   - P teleports (see teleport.go).
//   - Switches, each on its own function key, step a condition through its
//     settings on demand: the weather, the time of day, and whatever else a
//     plugin adds with addTestSwitch.
//
// A switch's first setting should be "as the game has it" (the weather's
// own schedule, the real clock), so pressing round its settings comes back
// to normal.
const testPanelKey = rl.KeyF1

// testSwitch is a condition the panel can switch, through Settings by its
// Key. Get is which setting is on; Set puts setting i on.
type testSwitch struct {
	Name     string
	Settings []string
	Key      input.Key // 0: the next free one from F5
	Get      func(w *ecs.World) int
	Set      func(w *ecs.World, i int)
}

// testSwitches is a resource: the switches, and whether the panel shows.
type testSwitches struct {
	list  []testSwitch
	shown bool
}

// testKeys are the keys handed out to switches that don't name one: F3 and
// F4 are illusion's stats and colliders.
var testKeys = []input.Key{rl.KeyF5, rl.KeyF6, rl.KeyF7, rl.KeyF8, rl.KeyF9, rl.KeyF10, rl.KeyF11, rl.KeyF12}

// addTestSwitch adds a switch to the test panel. Call it from a plugin's
// Build, any time before the game runs.
func addTestSwitch(app *illusion.App, s testSwitch) {
	sw := ecs.GetResource[testSwitches](app.World)
	if sw == nil {
		sw = &testSwitches{}
		ecs.AddResource(app.World, sw)
	}
	if s.Key == 0 {
		taken := map[input.Key]bool{}
		for _, o := range sw.list {
			taken[o.Key] = true
		}
		for _, k := range testKeys {
			if !taken[k] {
				s.Key = k
				break
			}
		}
	}
	sw.list = append(sw.list, s)
}

// testPlugin is the test tools: the panel, its switches, and teleporting.
// It needs the game's map and weather.
type testPlugin struct{}

func (testPlugin) Build(app *illusion.App) {
	addTestSwitch(app, weatherSwitch())
	addTestSwitch(app, shadowSwitch())
	app.AddSystems(illusion.Update,
		illusion.Fn6(teleport).Before(character.Input),
		illusion.Fn3(flipSwitches).Before(character.Input),
	)
	app.AddSystems(illusion.Render, illusion.Fn5(drawTestPanel).InSet(render.Draw2D))
}

// Shadows can be lowered or turned off while playing to check the GPU cost
// on the current device, without rebuilding or restarting the game.
func shadowSwitch() testSwitch {
	return testSwitch{
		Name: "Shadows", Key: rl.KeyF7,
		Settings: []string{"Full", "Low", "Off"},
		Get: func(w *ecs.World) int {
			s := ecs.GetResource[render.Shadows](w)
			if s == nil || s.Size == 0 {
				return 2
			}
			if s.Size < shadowSize {
				return 1
			}
			return 0
		},
		Set: func(w *ecs.World, i int) {
			if s := ecs.GetResource[render.Shadows](w); s != nil {
				s.Size = []int32{shadowSize, shadowSize / 2, 0}[i]
			}
		},
	}
}

// weatherSwitch holds the weather: clear, dusty or a dust storm, or as the
// schedule has it (see weather.go).
func weatherSwitch() testSwitch {
	return testSwitch{
		Name:     "Weather",
		Settings: []string{"As scheduled", "Clear", "Dusty", "Dust storm"},
		Get: func(w *ecs.World) int {
			wt := ecs.GetResource[weather](w)
			switch {
			case wt == nil || wt.forced < 0:
				return 0
			case wt.forced == 0:
				return 1
			case wt.forced < 1:
				return 2
			}
			return 3
		},
		Set: func(w *ecs.World, i int) {
			if wt := ecs.GetResource[weather](w); wt != nil {
				wt.forced = []float32{-1, 0, .45, 1}[i]
			}
		},
	}
}

// flipSwitches shows and hides the panel, and steps each switch whose key
// is pressed on to its next setting.
func flipSwitches(keys *illusion.Res[input.Keys], sw *illusion.Res[testSwitches], w *illusion.World) {
	s, ok := sw.TryGet()
	if !ok {
		return
	}
	k := keys.Get()
	if k.JustPressed(testPanelKey) {
		s.shown = !s.shown
	}
	world := w.World
	for _, t := range s.list {
		if k.JustPressed(t.Key) && len(t.Settings) > 0 {
			t.Set(world, (t.Get(world)+1)%len(t.Settings))
		}
	}
}

// drawTestPanel draws the panel, if it's shown, under the frame rate: each
// switch's key, name and setting, and the teleport.
func drawTestPanel(win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], sw *illusion.Res[testSwitches], m *illusion.Res[menu], w *illusion.World) {
	s, ok := sw.TryGet()
	if !ok || !s.shown || m.Get().screen() != playing && m.Get().screen() != mapping {
		return
	}
	ww := win.Get()
	p := newPainter(fonts.Get(), ww)
	world := w.World
	type row struct{ key, name, setting string }
	rows := []row{{"P", "Teleport", "to the map's pointer, or the marked spot"}}
	for _, t := range s.list {
		setting := ""
		if i := t.Get(world); i >= 0 && i < len(t.Settings) {
			setting = t.Settings[i]
		}
		rows = append(rows, row{keyName(t.Key), t.Name, setting})
	}
	width, line := p.px(340), p.px(30)
	r := rl.Rectangle{X: float32(ww.Width) - width - p.px(14), Y: p.px(170), Width: width, Height: p.px(40) + line*float32(len(rows))}
	p.panel(r)
	p.text("Testing", rl.Vector2{X: r.X + p.px(14), Y: r.Y + p.px(10)}, 15, semibold, colAccent)
	y := r.Y + p.px(36)
	for _, rw := range rows {
		x := r.X + p.px(14)
		x += max(p.keycap(rw.key, rl.Vector2{X: x, Y: y}, 13), p.px(34)) + p.px(10)
		p.text(rw.name, rl.Vector2{X: x, Y: y + p.px(4)}, 14, semibold, colMuted)
		p.textIn(rw.setting, rl.Rectangle{X: x + p.px(80), Y: y, Width: r.X + r.Width - x - p.px(94), Height: p.px(24)}, 14, semibold, colText, left)
		y += line
	}
}

// keyName is how a key's written on a keycap.
func keyName(k input.Key) string {
	if k >= rl.KeyF1 && k <= rl.KeyF12 {
		return fmt.Sprintf("F%d", k-rl.KeyF1+1)
	}
	if k >= rl.KeyA && k <= rl.KeyZ {
		return string(rune('A' + k - rl.KeyA))
	}
	return fmt.Sprint(k)
}
