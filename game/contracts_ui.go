package game

import (
	"fmt"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

func drawContractOffer(p painter, l layout, focus int) {
	p.panel(l.panel)
	x, y := l.heading.X, l.heading.Y
	w := l.panel.Width - p.px(2*pad)
	line := func(text string, offset, size float32, weight weight, ink rl.Color) {
		p.textIn(text, rl.Rectangle{X: x, Y: y + p.px(offset), Width: w, Height: p.px(size + 8)}, size, weight, ink, left)
	}
	line("PAD-001  •  DAY LABOUR", 0, 13, semibold, colAccent)
	r := rl.Rectangle{X: x + w - p.px(124), Y: y, Width: p.px(124), Height: p.px(26)}
	for _, edge := range [][2]rl.Vector2{
		{{X: r.X, Y: r.Y}, {X: r.X + r.Width, Y: r.Y}},
		{{X: r.X + r.Width, Y: r.Y}, {X: r.X + r.Width, Y: r.Y + r.Height}},
		{{X: r.X + r.Width, Y: r.Y + r.Height}, {X: r.X, Y: r.Y + r.Height}},
		{{X: r.X, Y: r.Y + r.Height}, {X: r.X, Y: r.Y}},
	} {
		stroke(edge[0], edge[1], p.px(1), colAccent)
	}
	p.textIn("OFFER", r, 13, semibold, colAccent, centre)
	line("First filing", 28, 32, black, colText)
	line("Posted by Ada Vellér • Patience arrivals", 76, 15, semibold, colMuted)
	line("Welcome. Your passage is on my books: 2,000 marks.", 119, 19, regular, colText)
	line("Deliver this filing, and I will credit the first 150.", 146, 19, regular, colText)
	line("Carry a sealed arrival filing from the Pads to Landfall.", 194, 17, regular, colText)
	line("Hand it over at the marked Registrar counter.", 220, 17, regular, colText)

	for i, row := range []struct{ label, value string }{
		{"Pay", "150 marks credited to your passage debt"},
		{"Bond / licence", "None • open to Unlisted arrivals"},
		{"Deadline / penalty", "No time limit • no default penalty"},
	} {
		r := rl.Rectangle{X: x, Y: y + p.px(265+float32(i)*32), Width: w, Height: p.px(30)}
		p.textIn(row.label, rl.Rectangle{X: r.X, Y: r.Y, Width: p.px(150), Height: r.Height}, 14, semibold, colMuted, left)
		p.textIn(row.value, rl.Rectangle{X: r.X + p.px(156), Y: r.Y, Width: r.Width - p.px(156), Height: r.Height}, 15, semibold, colText, left)
	}

	line("Accepting collects the filing and marks your delivery.", 377, 15, semibold, colAccent)
	menuButtons(p, l, contractOffer, focus)
}

func marks(n int) string {
	if n < 1000 {
		return fmt.Sprint(n)
	}
	return fmt.Sprintf("%d,%03d", n/1000, n%1000)
}

// The play summary has exactly the clock/weather card's footprint.
// Creditor and repayment details live in the journal.
func drawAccountSummary(p painter, width float32, c *contracts) {
	size := clockRect(p)
	r := rl.Rectangle{X: width - p.px(16) - size.Width, Y: p.px(16), Width: size.Width, Height: size.Height}
	p.panel(r)
	for i, row := range []struct {
		label  string
		amount int
	}{{"Balance", c.balance}, {"Debt", c.debt}} {
		y := r.Y + p.px(4+float32(i)*23)
		p.textIn(row.label, rl.Rectangle{X: r.X + p.px(10), Y: y, Width: p.px(46), Height: p.px(19)}, 11, semibold, colMuted, left)
		ink := colText
		if i == 1 && c.debt > 0 {
			ink = colAccent
		}
		p.textIn(marks(row.amount)+" marks", rl.Rectangle{X: r.X + p.px(58), Y: y, Width: r.Width - p.px(68), Height: p.px(19)}, 14, semibold, ink, right)
	}
}

// Keep current work under the minimap and the account in the top right.
// Completed work moves to the journal instead of staying in the task HUD.
func drawContractHUD(
	win *illusion.Res[window.Window],
	fonts *illusion.Res[uiFonts],
	m *illusion.Res[menu],
	jobs *illusion.Res[contracts],
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	driving *illusion.Res[vehicle.Driving],
) {
	c, ok := jobs.TryGet()
	if !ok || m.Get().screen() != playing {
		return
	}
	p := newPainter(fonts.Get(), win.Get())
	drawAccountSummary(p, float32(win.Get().Width), c)
	if c.state == contractDelivered {
		return
	}
	_, tr, ok := players.Single()
	if !ok {
		return
	}
	clock := clockRect(p)
	r := rl.Rectangle{X: minimapRect(p).X, Y: clock.Y + clock.Height + p.px(14), Width: p.px(310), Height: p.px(102)}
	p.panel(r)
	x, y := r.X+p.px(14), r.Y+p.px(12)
	text := func(value string, offset, size float32, ink rl.Color) {
		p.textIn(value, rl.Rectangle{X: x, Y: y + p.px(offset), Width: r.Width - p.px(28), Height: p.px(24)}, size, semibold, ink, left)
	}
	text("First filing", 0, 19, colAccent)
	switch c.state {
	case contractAvailable:
		text("Ada has work at the arrivals terminal.", 30, 14, colText)
		text("150 marks off your debt • no bond", 56, 14, colMuted)
	case contractAccepted:
		text("Accepted • deliver to the Exchange", 30, 14, colText)
		hint := "Carrying: sealed arrival filing"
		if driving.Get().Active() {
			hint = "Park outside the Hull; deliver on foot."
		} else if rl.Vector2Distance(rl.Vector2{X: tr.Translation.X, Y: tr.Translation.Z}, rl.Vector2{X: c.delivery.X, Y: c.delivery.Z}) > 100 {
			hint = "Follow the caravan road west to Landfall."
		}
		text(hint, 56, 14, colMuted)
	}
}
