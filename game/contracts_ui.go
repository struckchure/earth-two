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

// drawContractOffer draws the offer as the contract it is: the Exchange's
// form for filing one, its number boxed, the poster's terms in their own
// words, the terms in fields, the poster's signature and the seal of the
// licence it's open to; accepting it files it (a FILED stamp, menu.go).
func drawContractOffer(p painter, l layout, focus int) {
	p.paper(l.panel)
	x, w := l.heading.X, l.panel.Width-p.px(2*pad)
	y := p.formHeader(rl.Rectangle{X: x, Y: l.heading.Y, Width: w, Height: p.px(90)}, "THE EXCHANGE  •  CONTRACT FOR FILING", "PAD-001", "First filing", 30)
	at := func(dy, h float32) rl.Rectangle {
		return rl.Rectangle{X: x, Y: y + p.px(dy), Width: w, Height: p.px(h)}
	}
	p.textIn("Day labour • posted at Patience arrivals, the Pads", at(0, 18), 12, semibold, ledgerMuted, left)
	// The poster's terms, in their words, set off by a rule.
	rl.DrawRectangleRec(rl.Rectangle{X: x, Y: y + p.px(28), Width: max(2, p.px(2)), Height: p.px(48)}, ledgerRule)
	quote := func(dy float32, text string) {
		p.textIn(text, rl.Rectangle{X: x + p.px(14), Y: y + p.px(dy), Width: w - p.px(14), Height: p.px(22)}, 15, typed, ledgerInk, left)
	}
	quote(28, "“Welcome. Your passage is on my books: 2,000 marks.")
	quote(52, "Deliver this filing, and I will credit the first 150.”")
	p.textIn("The work", at(90, 14), 10, semibold, ledgerMuted, left)
	p.textIn("Carry a sealed arrival filing from the Pads to Landfall, and", at(104, 20), 14, typed, ledgerInk, left)
	p.textIn("hand it over at the marked Registrar counter.", at(124, 20), 14, typed, ledgerInk, left)
	// The terms, two to a row.
	half := (w - p.px(16)) / 2
	for i, f := range []struct{ label, value string }{
		{"Pay", "150 marks off the debt"},
		{"Bond", "None"},
		{"Deadline", "None"},
		{"Penalty on default", "None"},
	} {
		col, row := float32(i%2), float32(i/2)
		p.field(rl.Rectangle{X: x + col*(half+p.px(16)), Y: y + p.px(156+row*44), Width: half, Height: p.px(38)}, f.label, f.value)
	}
	// Signed by the poster; the taker signs by ticking Accept. The seal is
	// the licence it's open to.
	sy := y + p.px(252)
	p.textIn("Poster", rl.Rectangle{X: x, Y: sy, Width: half, Height: p.px(14)}, 10, semibold, ledgerMuted, left)
	p.textIn("A. Vellér", rl.Rectangle{X: x + p.px(6), Y: sy + p.px(12), Width: half, Height: p.px(26)}, 22, typedBold, tickInk, left)
	rl.DrawRectangleRec(rl.Rectangle{X: x, Y: sy + p.px(40), Width: half * .8, Height: max(1, p.px(1))}, ledgerInk)
	p.textIn("Licence: Unlisted and up", rl.Rectangle{X: x + half + p.px(16), Y: sy + p.px(6), Width: half - p.px(70), Height: p.px(30)}, 12, typed, ledgerMuted, left)
	drawSeal(p, rl.Vector2{X: x + w - p.px(30), Y: sy + p.px(22)}, 28, 0, "UNLISTED")
	paperChoices(p, l, contractOffer, focus)
}

func marks(n int) string {
	if n < 1000 {
		return fmt.Sprint(n)
	}
	return fmt.Sprintf("%d,%03d", n/1000, n%1000)
}

// The play summary has exactly the clock/weather card's footprint.
// Creditor and repayment details live in the journal.
func accountSummaryRect(p painter, width float32) rl.Rectangle {
	size := clockRect(p)
	return rl.Rectangle{X: width - p.px(16) - size.Width, Y: p.px(16), Width: size.Width, Height: size.Height}
}

func accountBadgeRect(p painter, card rl.Rectangle) rl.Rectangle {
	return rl.Rectangle{X: card.X - p.px(9), Y: card.Y - p.px(9), Width: p.px(18), Height: p.px(18)}
}

func drawAccountSummary(p painter, width float32, c *contracts) {
	r := accountSummaryRect(p, width)
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
	if count := len(c.availableContracts()); count > 0 {
		badge := accountBadgeRect(p, r)
		rl.DrawRectangleRec(inset(badge, -p.px(1), -p.px(1)), colPanel)
		rl.DrawRectangleRec(badge, colAccent)
		p.textIn(fmt.Sprint(count), badge, 12, semibold, colOnLight, centre)
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
	if c.ongoing == nil {
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
	text(c.ongoing.title, 0, 19, colAccent)
	text("Accepted • deliver to the Exchange", 30, 14, colText)
	hint := "Carrying: sealed arrival filing"
	if driving.Get().Active() {
		hint = "Park outside the Hull; deliver on foot."
	} else if rl.Vector2Distance(rl.Vector2{X: tr.Translation.X, Y: tr.Translation.Z}, rl.Vector2{X: c.delivery.X, Y: c.delivery.Z}) > 100 {
		hint = "Follow the caravan road west to Landfall."
	}
	text(hint, 56, 14, colMuted)
}
