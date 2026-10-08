//go:build identitypreview && !js

package game

import (
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/window"
)

// PreviewIdentity renders the actual in-game identity form without loading the
// 3D world. Available only in the opt-in identitypreview development build.
func PreviewIdentity() {
	rl.InitWindow(1000, 900, "Earth Two · Identity preview")
	defer rl.CloseWindow()
	rl.SetTargetFPS(60)
	setPaperCursorHidden(true)
	panel := newIdentityPanel()
	defer panel.close()
	fonts := &uiFonts{}
	m := &menu{}
	win := &window.Window{Width: 1000, Height: 900, Scale: 1}
	until := time.Now().Add(2 * time.Minute)
	for !rl.WindowShouldClose() && time.Now().Before(until) {
		rl.BeginDrawing()
		rl.ClearBackground(rl.NewColor(30, 28, 26, 255))
		p := newPainter(fonts, win)
		l := layoutFor(identityScreen, 1000, 900, p.s)
		drawIdentity(p, l, 0, panel)
		drawConnectionHUD(p, 900, nil)
		m.paintPaperCursor(rl.GetMousePosition(), p.s*.75)
		rl.EndDrawing()
	}
}
