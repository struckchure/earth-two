//go:build js

package game

// holdCursor can't hold the cursor in the browser: illusion's raylib for
// the web has no pointer lock yet. There the mouse turns the camera while a
// button is held.
func holdCursor(bool) bool { return false }
