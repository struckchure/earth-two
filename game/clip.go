package game

import rl "github.com/gen2brain/raylib-go/raylib"

// setClipPlanes sets the camera's clip planes (clipNear and clipFar, in
// camera.go). It must run once the window is open.
func setClipPlanes() { rl.SetClipPlanes(clipNear, clipFar) }
