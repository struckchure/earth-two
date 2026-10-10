package game

import (
	"syscall/js"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/window"
)

type touchPlugin struct{}

func (touchPlugin) Build(app *illusion.App) {
	// Own the merged state so holding a touch key doesn't become a new press
	// every frame when raylib reports no corresponding physical key.
	app.InsertResource(illusion.R(&input.Settings{Manual: true}))
	app.AddSystems(illusion.PreUpdate, illusion.Fn5(sampleBrowserInput).After(input.Sample))
}

type browserInputState struct{ physical, held map[input.Key]bool }

func sampleBrowserInput(keys *illusion.Res[input.Keys], buttons *illusion.Res[input.MouseButtons], mouse *illusion.Res[input.Mouse], win *illusion.Res[window.Window], state *illusion.Local[browserInputState]) {
	s := state.Get()
	if s.physical == nil {
		s.physical = map[input.Key]bool{}
		s.held = map[input.Key]bool{}
	}
	for key := range s.physical {
		if !rl.IsKeyDown(int32(key)) {
			delete(s.physical, key)
		}
	}
	for key := rl.GetKeyPressed(); key != 0; key = rl.GetKeyPressed() {
		s.physical[input.Key(key)] = true
	}
	merged := map[input.Key]bool{}
	for key := range s.physical {
		merged[key] = true
	}
	touch := js.Global().Get("earthTwoTouch")
	var snapshot js.Value
	active := false
	if !touch.IsUndefined() {
		snapshot = touch.Call("sample")
		active = snapshot.Get("active").Bool()
		list := snapshot.Get("keys")
		for i := 0; i < list.Length(); i++ {
			merged[input.Key(list.Index(i).Int())] = true
		}
	}
	for key := range s.held {
		if !merged[key] {
			keys.Get().Release(key)
		}
	}
	for key := range merged {
		keys.Get().Press(key)
	}
	s.held = merged
	b, m := buttons.Get(), mouse.Get()
	if active {
		p := snapshot.Get("mouse")
		w := win.Get()
		m.Position = rl.Vector2{X: float32(p.Get("x").Float()) * float32(w.Width), Y: float32(p.Get("y").Float()) * float32(w.Height)}
		m.Delta = rl.Vector2{X: float32(p.Get("dx").Float()) * float32(w.Width), Y: float32(p.Get("dy").Float()) * float32(w.Height)}
		m.Wheel = float32(snapshot.Get("wheel").Float())
		for button := rl.MouseButtonLeft; button <= rl.MouseButtonBack; button++ {
			if button == rl.MouseButtonLeft && p.Get("down").Bool() {
				b.Press(button)
			} else {
				b.Release(button)
			}
		}
	} else {
		for button := rl.MouseButtonLeft; button <= rl.MouseButtonBack; button++ {
			if rl.IsMouseButtonDown(button) {
				b.Press(button)
			} else {
				b.Release(button)
			}
		}
		m.Position, m.Delta, m.Wheel = rl.GetMousePosition(), rl.GetMouseDelta(), rl.GetMouseWheelMove()
	}
}

func touchActive() bool {
	touch := js.Global().Get("earthTwoTouch")
	return !touch.IsUndefined() && touch.Call("active").Bool()
}

func syncTouchInteract(available bool, label string) {
	if touch := js.Global().Get("earthTwoTouch"); !touch.IsUndefined() {
		touch.Call("setInteract", available, label)
	}
}

func syncTouchContext(driving, running, sliding, pickup, lights bool) {
	if touch := js.Global().Get("earthTwoTouch"); !touch.IsUndefined() {
		touch.Call("setContext", driving, running, sliding, pickup, lights)
	}
}

func syncTouchHUD(x, y, width, height, cardBottom float32) {
	if touch := js.Global().Get("earthTwoTouch"); !touch.IsUndefined() {
		touch.Call("setHUD", x, y, width, height, cardBottom)
	}
}
