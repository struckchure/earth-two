//go:build !js

package game

import rl "github.com/gen2brain/raylib-go/raylib"

// drawScale is how many screen pixels raylib draws each 2D pixel with: the
// display's scale, with HighDPI.
func drawScale() float32 { return rl.GetWindowScaleDPI().X }
