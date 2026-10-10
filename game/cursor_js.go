//go:build js

package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/window"
	"syscall/js"
)

// The browser shell handles the user gesture required for pointer lock.
func holdCursor(hold bool) bool {
	pointer := js.Global().Get("earthTwoPointer")
	if pointer.IsUndefined() {
		return false
	}
	pointer.Call("setPlaying", hold)
	return pointer.Call("locked").Bool()
}

func cursorDelta(fallback rl.Vector2) rl.Vector2 {
	pointer := js.Global().Get("earthTwoPointer")
	if pointer.IsUndefined() || !pointer.Call("locked").Bool() {
		return fallback
	}
	delta := pointer.Call("takeDelta")
	return rl.Vector2{X: float32(delta.Index(0).Float()), Y: float32(delta.Index(1).Float())}
}

func cursorReleased() bool {
	pointer := js.Global().Get("earthTwoPointer")
	return !pointer.IsUndefined() && pointer.Call("consumeUnlock").Bool()
}

func setPaperCursorHidden(active bool) {
	canvas := js.Global().Get("document").Call("getElementById", "canvas")
	if canvas.IsNull() {
		return
	}
	if active {
		canvas.Get("style").Set("cursor", "none")
	} else {
		canvas.Get("style").Set("cursor", "")
	}
}
func paperCursorInside(_ rl.Vector2, _ *window.Window) bool {
	canvas := js.Global().Get("document").Call("getElementById", "canvas")
	return !canvas.IsNull() && canvas.Call("matches", ":hover").Bool()
}
