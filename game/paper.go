package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/window"
)

// Paperwork: the menus' look (docs/look-and-feel.md: "menus look like filed
// forms, stamps confirm actions, and Registrar seals mark licence tiers").
// A menu is a sheet of the Exchange's paper, punched for its file: a form
// header with who issued it and its number, values typed in Courier Prime,
// and its choices boxes to tick on ruled lines, the one in hand ticked and
// gone over with a highlighter. Confirming something stamps it, an inked
// rubber stamp come down on it (FILED, ADMITTED, SETTLED). A licence tier
// is a Registrar's brass seal. Square corners, as everything.

// The paper and its inks: the journal's (ledger.go), and the stamps' and
// seals'.
var (
	paperEdge   = rl.NewColor(204, 192, 166, 255)
	paperShadow = rl.NewColor(0, 0, 0, 90)
	paperGrain  = rl.NewColor(120, 104, 78, 255)
	tickInk     = rl.NewColor(38, 58, 128, 255)
	highlighter = rl.NewColor(255, 200, 80, 120)
	stampRed    = rl.NewColor(172, 38, 32, 255)
	stampBlue   = rl.NewColor(36, 62, 138, 255)
	sealBrass   = rl.NewColor(176, 136, 62, 255)
	sealDark    = rl.NewColor(110, 80, 32, 255)
)

// paper draws a sheet over r: a shadow under it, the stock with its grain
// and fibres, a darker edge, and the two holes it was punched with to file
// it, down its left margin.
func (p painter) paper(r rl.Rectangle) {
	off := p.px(6)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X + off, Y: r.Y + off, Width: r.Width, Height: r.Height}, paperShadow)
	rl.DrawRectangleRec(r, ledgerPaper)
	// Grain: specks and short fibres, the same on every sheet the same
	// size, faint.
	n := int(r.Width * r.Height / (p.px(14) * p.px(14)))
	for i := range n {
		u, v := lattice(int32(i), 1, 0x9a9e), lattice(int32(i), 2, 0x9a9e)
		k := lattice(int32(i), 3, 0x9a9e)
		c := paperGrain
		c.A = uint8(10 + 22*k)
		w, h := max(1, p.px(1)), max(1, p.px(1))
		if k > .8 {
			w = p.px(2 + 5*k) // a fibre
		}
		rl.DrawRectangleRec(rl.Rectangle{X: r.X + u*(r.Width-w), Y: r.Y + v*(r.Height-h), Width: w, Height: h}, c)
	}
	edge := max(1, p.px(1))
	outline(r, edge, paperEdge)
	// The punched holes, showing what's behind the sheet darker.
	for _, f := range []float32{.22, .78} {
		if r.Height < p.px(160) {
			break
		}
		circle(r.X+p.px(14), r.Y+r.Height*f, p.px(5), rl.NewColor(30, 26, 22, 200))
	}
}

// formHeader heads a form at the top of r: the issuer in small capitals at
// the left, the form's number boxed at the right, the title large under
// them, and a double rule; it returns where under it the form goes on.
func (p painter) formHeader(r rl.Rectangle, issuer, number, title string, titleSize float32) float32 {
	x, y, w := r.X, r.Y, r.Width
	p.textIn(issuer, rl.Rectangle{X: x, Y: y, Width: w * .7, Height: p.px(16)}, 11, semibold, ledgerMuted, left)
	if number != "" {
		m := p.measure(number, 11, typedBold)
		box := rl.Rectangle{X: x + w - m.X - p.px(14), Y: y - p.px(2), Width: m.X + p.px(14), Height: p.px(20)}
		outline(box, max(1, p.px(1)), ledgerInk)
		p.textIn(number, box, 11, typedBold, ledgerInk, centre)
	}
	y += p.px(22)
	p.textIn(title, rl.Rectangle{X: x, Y: y, Width: w, Height: p.px(titleSize * 1.25)}, titleSize, black, ledgerInk, left)
	y += p.px(titleSize*1.25 + 6)
	rule := max(1, p.px(1))
	rl.DrawRectangleRec(rl.Rectangle{X: x, Y: y, Width: w, Height: rule}, ledgerInk)
	rl.DrawRectangleRec(rl.Rectangle{X: x, Y: y + p.px(3), Width: w, Height: rule}, ledgerInk)
	return y + p.px(12)
}

// field draws a form's field across r: its label in small capitals over
// the value typed on a ruled line.
func (p painter) field(r rl.Rectangle, label, value string) {
	p.textIn(label, rl.Rectangle{X: r.X, Y: r.Y, Width: r.Width, Height: p.px(14)}, 10, semibold, ledgerMuted, left)
	p.textIn(value, rl.Rectangle{X: r.X, Y: r.Y + p.px(14), Width: r.Width, Height: r.Height - p.px(16)}, 15, typed, ledgerInk, left)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y + r.Height - p.px(1), Width: r.Width, Height: max(1, p.px(1))}, ledgerRule)
}

// tickChoice draws a choice on a form in r: a box to tick and its label
// typed beside it on a ruled line; the one in hand is ticked, in ink, and
// gone over with a highlighter.
func (p painter) tickChoice(r rl.Rectangle, label string, hot bool) {
	if hot {
		rl.DrawRectangleRec(rl.Rectangle{X: r.X + p.px(34), Y: r.Y + r.Height*.2, Width: r.Width - p.px(34), Height: r.Height * .6}, highlighter)
	}
	box := rl.Rectangle{X: r.X + p.px(6), Y: r.Y + (r.Height-p.px(18))/2, Width: p.px(18), Height: p.px(18)}
	line := max(1, p.px(1.5))
	outline(box, line, ledgerInk)
	if hot {
		// A tick, in two strokes of the pen.
		a := rl.Vector2{X: box.X + p.px(3), Y: box.Y + p.px(9)}
		b := rl.Vector2{X: box.X + p.px(7.5), Y: box.Y + p.px(14)}
		c := rl.Vector2{X: box.X + p.px(17), Y: box.Y - p.px(2)}
		stroke(a, b, p.px(2.5), tickInk)
		stroke(b, c, p.px(2.5), tickInk)
	}
	p.textIn(label, rl.Rectangle{X: r.X + p.px(40), Y: r.Y, Width: r.Width - p.px(46), Height: r.Height}, 18, typedBold, ledgerInk, left)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y + r.Height - max(1, p.px(1)), Width: r.Width, Height: max(1, p.px(1))}, ledgerRule)
}

// outline draws r's edges, thick wide, inside it (the browser's raylib has
// no outlined rectangle of a width).
func outline(r rl.Rectangle, thick float32, c rl.Color) {
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y, Width: r.Width, Height: thick}, c)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y + r.Height - thick, Width: r.Width, Height: thick}, c)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y, Width: thick, Height: r.Height}, c)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X + r.Width - thick, Y: r.Y, Width: thick, Height: r.Height}, c)
}

// circle draws a filled circle at (x, y), radius across.
func circle(x, y, radius float32, c rl.Color) {
	rl.DrawCircle(int32(math.Round(float64(x))), int32(math.Round(float64(y))), radius, c)
}

// paperChoices draws screen s's choices on its form.
func paperChoices(p painter, l layout, s screen, focus int) {
	for i, it := range items(s) {
		p.tickChoice(l.buttons[i], it.label, i == focus)
	}
}

// drawStampMark draws a rubber stamp's mark: text in heavy capitals inside
// a double border, centred on at, turned by turn (degrees), size points
// high, in ink at alpha (0 to 1). On paper, worn: the ink's missed in
// specks.
func drawStampMark(p painter, text string, at rl.Vector2, turn, size float32, ink rl.Color, alpha float32, onPaper bool) {
	f, px := p.font(size, black)
	m := rl.MeasureTextEx(f, text, px, px*.08)
	w, h := m.X+p.px(size*.9), m.Y+p.px(size*.45)
	c := ink
	c.A = uint8(float32(ink.A) * alpha * .9)
	// The borders: each edge a bar turned with the whole, about at.
	bar := func(x, y, bw, bh float32) {
		rl.DrawRectanglePro(rl.Rectangle{X: at.X, Y: at.Y, Width: bw, Height: bh}, rl.Vector2{X: -x, Y: -y}, turn, c)
	}
	for _, in := range []float32{0, p.px(size * .16)} {
		t := p.px(size * .11)
		if in > 0 {
			t = p.px(size * .05)
		}
		x0, y0, x1, y1 := -w/2+in, -h/2+in, w/2-in, h/2-in
		bar(x0, y0, x1-x0, t)
		bar(x0, y1-t, x1-x0, t)
		bar(x0, y0, t, y1-y0)
		bar(x1-t, y0, t, y1-y0)
	}
	rl.DrawTextPro(f, text, at, rl.Vector2{X: m.X / 2, Y: m.Y / 2}, turn, px, px*.08, c)
	if !onPaper {
		return
	}
	// Worn: specks of the paper showing through.
	rad := float64(turn) * math.Pi / 180
	cos, sin := float32(math.Cos(rad)), float32(math.Sin(rad))
	speck := ledgerPaper
	speck.A = uint8(200 * alpha)
	for i := range int(w * h / (p.px(9) * p.px(9))) {
		u, v := (lattice(int32(i), 4, 0x57a9)-.5)*w, (lattice(int32(i), 5, 0x57a9)-.5)*h
		s := max(1, p.px(1+1.5*lattice(int32(i), 6, 0x57a9)))
		rl.DrawRectangleRec(rl.Rectangle{X: at.X + u*cos - v*sin, Y: at.Y + u*sin + v*cos, Width: s, Height: s}, speck)
	}
}

// drawSeal draws a Registrar's seal centred on at, radius points across: a
// brass disc with a raised rim, the registry's name round it, and the
// licence tier's numeral and title in the middle.
func drawSeal(p painter, at rl.Vector2, radius float32, tier int, name string) {
	r := p.px(radius)
	circle(at.X+p.px(1.5), at.Y+p.px(2), r, rl.NewColor(0, 0, 0, 70))
	circle(at.X, at.Y, r, sealDark)
	circle(at.X, at.Y, r*.94, sealBrass)
	circle(at.X, at.Y, r*.66, sealDark)
	circle(at.X, at.Y, r*.62, sealBrass)
	// The registry's name round the rim, a letter at a time.
	text := "REGISTRY OF LANDFALL * EXCHANGE * "
	f, px := p.font(radius*.17, black)
	n := len([]rune(text))
	for i, ch := range text {
		a := float64(i)/float64(n)*2*math.Pi - math.Pi/2
		pos := rl.Vector2{X: at.X + float32(math.Cos(a))*r*.8, Y: at.Y + float32(math.Sin(a))*r*.8}
		rl.DrawTextPro(f, string(ch), pos, rl.Vector2{X: px * .3, Y: px * .5}, float32(a*180/math.Pi)+90, px, 0, sealDark)
	}
	numeral := []string{"0", "I", "II", "III", "IV", "V"}[max(0, min(tier, 5))]
	p.textIn(numeral, rl.Rectangle{X: at.X - r, Y: at.Y - r*.56, Width: 2 * r, Height: r * .8}, radius*.62, black, sealDark, centre)
	p.textIn(name, rl.Rectangle{X: at.X - r*.62, Y: at.Y + r*.16, Width: r * 1.24, Height: r * .3}, radius*.2, black, sealDark, centre)
}

// A stamp come down to confirm something, over whatever's on screen: on
// the form it was done on (from), where that was, even once it's gone.
type stampMark struct {
	text   string
	ink    rl.Color
	from   screen
	focus  int // the choice ticked on it
	turn   float32
	age    float32
	at     rl.Vector2
	sheet  rl.Rectangle
	placed bool
}

// stampLife is how long a stamp's mark stays: it comes down over the first
// stampDrop seconds, and fades over the last stampFade.
const stampLife, stampDrop, stampFade = 1.5, .14, .45

// stampOn has a stamp come down with text in ink, on screen from's form,
// turned a little, a different little for each.
func (m *menu) stampOn(text string, ink rl.Color, from screen) {
	turn := -8 + 6*lattice(int32(len(text)), int32(from), 0x5a3)
	focus := -1
	if len(m.stack) > 0 && m.top().screen == from {
		focus = m.top().focus
	}
	m.stamp = &stampMark{text: text, ink: ink, from: from, focus: focus, turn: turn}
}

// stampSet draws the stamps over everything else.
const stampSet illusion.SystemSet = "game.stamp"

// drawStamps draws the stamp that's come down, if one has, and ages it.
func drawStamps(m *illusion.Res[menu], win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], clk *illusion.Res[illusion.Time]) {
	mu := m.Get()
	s := mu.stamp
	if s == nil {
		return
	}
	s.age += clk.Get().DeltaSecs()
	if s.age > stampLife {
		mu.stamp = nil
		return
	}
	ww := win.Get()
	p := newPainter(fonts.Get(), ww)
	if !s.placed {
		// Where its form was: the middle of its sheet.
		l := layoutFor(s.from, float32(ww.Width), float32(ww.Height), p.s)
		s.sheet = l.panel
		s.at, s.placed = rl.Vector2{X: l.panel.X + l.panel.Width/2, Y: l.panel.Y + l.panel.Height*.4}, true
	}
	drop := smoothstep(0, stampDrop, s.age)
	scale := 1 + .7*(1-drop)
	at, alpha, onPaper := s.at, drop, mu.screen() == s.from
	if onPaper {
		// On the form, still up: it fades from it.
		alpha *= 1 - smoothstep(stampLife-stampFade, stampLife, s.age)
	} else {
		// The form's gone (filed): a sheet of it under the stamp, taken
		// away down off the screen once it's stamped.
		k := smoothstep(stampDrop+.4, stampLife, s.age)
		away := k * k * (float32(ww.Height) - s.sheet.Y + p.px(40))
		if paperForm(s.from) {
			drawForm(p, layoutFor(s.from, float32(ww.Width), float32(ww.Height), p.s).down(away), s.from, s.focus)
		} else {
			p.paper(rl.Rectangle{X: s.sheet.X, Y: s.sheet.Y + away, Width: s.sheet.Width, Height: s.sheet.Height})
		}
		at.Y += away
	}
	drawStampMark(p, s.text, at, s.turn, 46*scale, s.ink, alpha, true)
}
