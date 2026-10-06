package game

import rl "github.com/gen2brain/raylib-go/raylib"

// clipNear and clipFar are how near and far the camera sees. raylib's
// near default, 0.01 m, leaves the depth buffer too coarse for the pieces'
// painted-on details (stains, stripes, panel seams a millimetre or two off
// the surface under them): past 15 m or so they flickered through it. Ten
// times further, it's ten times finer everywhere. The spring arm keeps the
// camera further than this from anything.
const clipNear, clipFar = 0.1, 1000

// setClipPlanes sets them. It must run once the window is open.
func setClipPlanes() { rl.SetClipPlanes(clipNear, clipFar) }
