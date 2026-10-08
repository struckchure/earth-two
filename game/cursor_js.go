//go:build js

package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/window"
	"syscall/js"
)

// holdCursor can't hold the cursor in the browser: illusion's raylib for
// the web has no pointer lock yet. There the mouse turns the camera while a
// button is held.
func holdCursor(bool) bool { return false }

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
