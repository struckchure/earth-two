package game

import (
	"embed"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/window"
)

// The look shared by the menus and the HUD: Inter in three weights, dark
// see-through panels with square corners and a warm amber accent. Sizes are in points, which
// uiScale turns into pixels for the window.

//go:embed fonts/*.ttf
var fontFiles embed.FS

type weight uint8

const (
	regular weight = iota
	semibold
	black
)

var fontPaths = [...]string{"fonts/Inter-Regular.ttf", "fonts/Inter-SemiBold.ttf", "fonts/Inter-Black.ttf"}

// glyphs are the characters loaded from the fonts: Latin-1, the bullet, the
// ellipsis and the arrows. Not the soft hyphen, which Inter hasn't got, or
// the bar, which is taller than the font's size (raylib warns about it).
var glyphs = func() []rune {
	var out []rune
	for _, span := range [][2]rune{{0x20, 0x7e}, {0xa0, 0xff}, {0x2022, 0x2022}, {0x2026, 0x2026}, {0x2190, 0x2193}} {
		for r := span[0]; r <= span[1]; r++ {
			if r != '|' && r != 0xad {
				out = append(out, r)
			}
		}
	}
	return out
}()

var (
	colText    = rl.NewColor(245, 240, 232, 255)
	colMuted   = rl.NewColor(178, 168, 156, 255)
	colAccent  = rl.NewColor(255, 184, 82, 255)
	colOnLight = rl.NewColor(28, 22, 16, 255)
	colPanel   = rl.NewColor(16, 13, 11, 214)
	colControl = rl.NewColor(255, 255, 255, 22)
	colHover   = rl.NewColor(255, 255, 255, 44)
)

// uiScale is pixels per point: the window's pixel ratio, shrunk on small
// windows and grown on big ones so the menus keep their proportions.
func uiScale(w *window.Window) float32 {
	s := w.Scale
	if s <= 0 {
		s = 1
	}
	fit := min(float32(w.Height)/s/760, float32(w.Width)/s/1150)
	return s * min(max(fit, 0.7), 1.6)
}

type fontKey struct {
	w  weight
	px int32
}

// uiFonts is a resource: the fonts, each loaded the first time it's drawn
// at a size, at that size in the display's own pixels (rounded up a little,
// so resizing the window loads a few sizes, not one for every pixel), so text
// is sharp at any scale.
type uiFonts struct {
	loaded map[fontKey]rl.Font
}

func (f *uiFonts) get(w weight, px int32) rl.Font {
	k := fontKey{w, max(px, 6)}
	if font, ok := f.loaded[k]; ok {
		return font
	}
	if f.loaded == nil {
		f.loaded = map[fontKey]rl.Font{}
	}
	data, err := fontFiles.ReadFile(fontPaths[w])
	if err != nil {
		panic(err)
	}
	font := rl.LoadFontFromMemory(".ttf", data, k.px, glyphs)
	rl.SetTextureFilter(font.Texture, rl.FilterBilinear)
	f.loaded[k] = font
	return font
}

// rasterSize is the size to load a font at to draw it px display pixels
// high: px rounded up to a step that grows with it.
func rasterSize(px float32) int32 {
	n := int32(math.Ceil(float64(px)))
	step := int32(1)
	switch {
	case n > 64:
		step = 8
	case n > 32:
		step = 4
	case n > 16:
		step = 2
	}
	return (n + step - 1) / step * step
}

// painter draws the UI at a scale.
type painter struct {
	fonts *uiFonts
	s     float32
	// dpi is how many display pixels raylib draws each of ours with: on a
	// HighDPI desktop screen it scales all 2D drawing itself.
	dpi float32
}

func newPainter(fonts *uiFonts, w *window.Window) painter {
	return painter{fonts: fonts, s: uiScale(w), dpi: drawScale()}
}

func (p painter) px(pt float32) float32 { return pt * p.s }

// font is the font to draw text size points high with, and its size in our
// pixels.
func (p painter) font(size float32, w weight) (rl.Font, float32) {
	px := float32(math.Round(float64(size * p.s)))
	return p.fonts.get(w, rasterSize(px*max(p.dpi, 1))), px
}

// measure is text's size in pixels.
func (p painter) measure(text string, size float32, w weight) rl.Vector2 {
	f, px := p.font(size, w)
	return rl.MeasureTextEx(f, text, px, 0)
}

// text draws text with its top left at pos, in pixels.
func (p painter) text(text string, pos rl.Vector2, size float32, w weight, c rl.Color) {
	f, px := p.font(size, w)
	rl.DrawTextEx(f, text, rl.Vector2{X: float32(math.Round(float64(pos.X))), Y: float32(math.Round(float64(pos.Y)))}, px, 0, c)
}

type align uint8

const (
	left align = iota
	centre
	right
)

// textIn draws text centred vertically in r, shrinking it to fit r's width.
func (p painter) textIn(text string, r rl.Rectangle, size float32, w weight, c rl.Color, a align) {
	m := p.measure(text, size, w)
	if m.X > r.Width && size > 9 {
		// Text's width goes with its size: one step to about the right
		// size, then nudge it down for the rounding.
		size = max(9, size*r.Width/m.X)
		for m = p.measure(text, size, w); size > 9 && m.X > r.Width; m = p.measure(text, size, w) {
			size = max(9, size-0.5)
		}
	}
	x := r.X
	switch a {
	case centre:
		x += (r.Width - m.X) / 2
	case right:
		x += r.Width - m.X
	}
	p.text(text, rl.Vector2{X: x, Y: r.Y + (r.Height-m.Y)/2}, size, w, c)
}

func (p painter) panel(r rl.Rectangle) {
	rl.DrawRectangleRec(r, colPanel)
}

// button draws a menu button; hot is under the mouse or the keyboard's focus.
func (p painter) button(r rl.Rectangle, label string, hot bool) {
	fill, ink := colControl, colText
	if hot {
		fill, ink = colAccent, colOnLight
	}
	rl.DrawRectangleRec(r, fill)
	p.textIn(label, inset(r, p.px(20), 0), 19, semibold, ink, left)
}

// keycap draws key as a small key at pos and returns how wide it is.
func (p painter) keycap(key string, pos rl.Vector2, size float32) float32 {
	m := p.measure(key, size, semibold)
	r := rl.Rectangle{X: pos.X, Y: pos.Y, Width: max(m.X+p.px(14), p.px(size*1.7)), Height: p.px(size * 1.7)}
	rl.DrawRectangleRec(r, rl.NewColor(255, 255, 255, 80))
	rl.DrawRectangleRec(inset(r, max(1, p.px(1)), max(1, p.px(1))), rl.NewColor(58, 52, 46, 255))
	p.textIn(key, r, size, semibold, colText, centre)
	return r.Width
}

// chevron draws an arrow pointing left (dir -1) or right (+1) centred in r.
func (p painter) chevron(r rl.Rectangle, dir float32, c rl.Color) {
	cx, cy := r.X+r.Width/2, r.Y+r.Height/2
	h := min(r.Width, r.Height) * 0.2
	tip := rl.Vector2{X: cx + dir*h/2, Y: cy}
	thick := max(1.5, p.px(2.4))
	stroke(rl.Vector2{X: cx - dir*h/2, Y: cy - h}, tip, thick, c)
	stroke(rl.Vector2{X: cx - dir*h/2, Y: cy + h}, tip, thick, c)
	rl.DrawCircle(int32(tip.X), int32(tip.Y), thick/2, c)
}

// stroke draws a thick line from a to b. (The browser build's raylib has
// no DrawLineEx, so it's a rotated rectangle.)
func stroke(a, b rl.Vector2, thick float32, c rl.Color) {
	d := rl.Vector2Subtract(b, a)
	angle := float32(math.Atan2(float64(d.Y), float64(d.X))) * 180 / math.Pi
	rl.DrawRectanglePro(rl.Rectangle{X: a.X, Y: a.Y, Width: rl.Vector2Length(d), Height: thick}, rl.Vector2{Y: thick / 2}, angle, c)
}

// shade darkens the window's left side, solid for the first third of
// width and fading out by width: a backdrop for the menus. (Two-pixel
// strips, as the browser build's raylib has no gradients.)
func shade(width, height float32, alpha uint8) {
	for x := int32(0); x < int32(width); x += 2 {
		t := float32(x) / width
		f := min(max((t-0.3)/0.7, 0), 1)
		a := float32(alpha) * (1 - f*f*(3-2*f))
		rl.DrawRectangle(x, 0, 2, int32(height)+1, rl.NewColor(12, 9, 7, uint8(a)))
	}
}

func inset(r rl.Rectangle, dx, dy float32) rl.Rectangle {
	return rl.Rectangle{X: r.X + dx, Y: r.Y + dy, Width: r.Width - 2*dx, Height: r.Height - 2*dy}
}

func contains(r rl.Rectangle, p rl.Vector2) bool { return rl.CheckCollisionPointRec(p, r) }
