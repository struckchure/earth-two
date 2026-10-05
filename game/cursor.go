//go:build !js

package game

import rl "github.com/gen2brain/raylib-go/raylib"

// cursorHeld is whether holdCursor has the cursor.
var cursorHeld bool

// holdCursor hides the cursor and keeps it in the window while hold is set,
// so the mouse turns the camera freely (see steerCamera), and lets it go
// otherwise, for the menus. It reports whether it has it, and a frame
// whose mouse movement counts: not the one it's taken in, whose movement
// is the jump to the middle of the window.
func holdCursor(hold bool) bool {
	if hold == cursorHeld {
		return hold
	}
	cursorHeld = hold
	if hold {
		rl.DisableCursor()
	} else {
		rl.EnableCursor()
	}
	return false
}
