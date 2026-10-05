//go:build js

package game

// setClipPlanes can't move the near plane in the browser yet: illusion's
// raylib for the web has no SetClipPlanes, so there the pieces' painted-on
// details may flicker at a distance.
func setClipPlanes() {}
