package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/window"
)

// paperCursorPixels paints a small folded parchment arrow. It is generated
// once, cached as a texture, and shared by desktop and browser rendering.
func paperCursorPixels() []byte {
	const w, h, ss = 36, 42, 3
	shape := []rl.Vector2{{X: 3, Y: 3}, {X: 3, Y: 29}, {X: 10, Y: 23}, {X: 15, Y: 35}, {X: 20, Y: 33}, {X: 15, Y: 22}, {X: 28, Y: 21}}
	inside := func(x, y float32) bool {
		in := false
		for i, a := range shape {
			b := shape[(i+1)%len(shape)]
			if (a.Y > y) != (b.Y > y) && x < (b.X-a.X)*(y-a.Y)/(b.Y-a.Y)+a.X {
				in = !in
			}
		}
		return in
	}
	pixels := make([]byte, w*h*ss*ss*4)
	for y := range h * ss {
		for x := range w * ss {
			px, py := (float32(x)+.5)/ss, (float32(y)+.5)/ss
			c := rl.Color{}
			if inside(px-1.8, py-1.8) {
				c = rl.NewColor(0, 0, 0, 70)
			}
			if inside(px, py) {
				c = rl.NewColor(246, 237, 211, 255)
				edge := float32(math.Inf(1))
				for i, a := range shape {
					edge = min(edge, segmentDistance(a, shape[(i+1)%len(shape)], px, py))
				}
				if edge < .8 {
					c = ledgerInk
				} else if px > 16 && py > 13 && py < 22 {
					c = rl.NewColor(211, 195, 160, 255)
				} else if (x/ss*13+y/ss*7)%23 == 0 {
					c = rl.NewColor(237, 226, 195, 255)
				}
			}
			i := (y*w*ss + x) * 4
			pixels[i], pixels[i+1], pixels[i+2], pixels[i+3] = c.R, c.G, c.B, c.A
		}
	}
	return pixels
}

func drawPaperCursor(menus *illusion.Res[menu], mouse *illusion.Res[input.Mouse], win *illusion.Res[window.Window]) {
	m, w := menus.Get(), win.Get()
	active := m.screen() != playing
	setPaperCursorHidden(active)
	if !active || !paperCursorInside(mouse.Get().Position, w) {
		return
	}
	m.paintPaperCursor(mouse.Get().Position, uiScale(w)*.75)
}

func (m *menu) paintPaperCursor(at rl.Vector2, scale float32) {
	if m.cursor.ID == 0 {
		m.cursor = uploadTexture(paperCursorPixels(), 36*3, 42*3).Texture2D
	}
	rl.DrawTexturePro(m.cursor, rl.Rectangle{Width: float32(m.cursor.Width), Height: float32(m.cursor.Height)}, rl.Rectangle{X: at.X - 3*scale, Y: at.Y - 3*scale, Width: 36 * scale, Height: 42 * scale}, rl.Vector2{}, 0, rl.White)
}
