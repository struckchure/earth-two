package game

import (
	"fmt"

	rl "github.com/gen2brain/raylib-go/raylib"
)

// The journal is a personal accounting book. Amounts are in marks, receipts
// keep their actual settlement, and the play HUD keeps its compact summary.
var (
	ledgerPaper  = rl.NewColor(236, 227, 207, 255)
	ledgerInk    = rl.NewColor(46, 48, 43, 255)
	ledgerMuted  = rl.NewColor(107, 106, 94, 255)
	ledgerRule   = rl.NewColor(150, 157, 143, 160)
	ledgerHead   = rl.NewColor(219, 216, 195, 255)
	ledgerDebit  = rl.NewColor(137, 71, 44, 255)
	ledgerCredit = rl.NewColor(49, 91, 74, 255)
)

const ledgerHistoryRows = 4

type ledgerColumn struct {
	name  string
	share float32
	align align
}

type ledgerCell struct {
	text, note string
	ink        rl.Color
}

// ledgerTable draws fixed ruled rows, keeping unused lines blank like a book.
// The final row contains totals for the whole account, not only visible rows.
func ledgerTable(p painter, r rl.Rectangle, columns []ledgerColumn, rows [][]ledgerCell, totals []ledgerCell, empty string) {
	const head, row, foot = 28, 52, 30
	edge := max(1, p.px(1))
	rl.DrawRectangleRec(r, ledgerPaper)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y, Width: r.Width, Height: p.px(head)}, ledgerHead)
	footerY := r.Y + r.Height - p.px(foot)
	rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: footerY, Width: r.Width, Height: p.px(foot)}, ledgerHead)
	x := r.X
	for col, column := range columns {
		width := r.Width * column.share
		cell := func(y, height float32) rl.Rectangle {
			return inset(rl.Rectangle{X: x, Y: y, Width: width, Height: height}, p.px(10), 0)
		}
		p.textIn(column.name, cell(r.Y, p.px(head)), 10, semibold, ledgerMuted, column.align)
		for i, record := range rows {
			if col >= len(record) {
				continue
			}
			y := r.Y + p.px(head+float32(i)*row)
			value := record[col]
			ink := value.ink
			if ink.A == 0 {
				ink = ledgerInk
			}
			if value.note == "" {
				p.textIn(value.text, cell(y, p.px(row)), 14, semibold, ink, column.align)
			} else {
				p.textIn(value.text, cell(y+p.px(5), p.px(24)), 14, semibold, ink, column.align)
				p.textIn(value.note, cell(y+p.px(29), p.px(18)), 11, regular, ledgerMuted, column.align)
			}
		}
		if col < len(totals) {
			ink := totals[col].ink
			if ink.A == 0 {
				ink = ledgerInk
			}
			p.textIn(totals[col].text, cell(footerY, p.px(foot)), 13, semibold, ink, column.align)
		}
		rl.DrawRectangleRec(rl.Rectangle{X: x, Y: r.Y, Width: edge, Height: r.Height}, ledgerRule)
		x += width
	}
	for y := r.Y + p.px(head); y < footerY; y += p.px(row) {
		rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: y, Width: r.Width, Height: edge}, ledgerRule)
	}
	for _, y := range []float32{r.Y, footerY, footerY + p.px(3), r.Y + r.Height - edge} {
		rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: y, Width: r.Width, Height: edge}, ledgerRule)
	}
	rl.DrawRectangleRec(rl.Rectangle{X: r.X + r.Width - edge, Y: r.Y, Width: edge, Height: r.Height}, ledgerRule)
	if len(rows) == 0 {
		// Erase vertical rules on the first unused row for the empty-state note.
		blank := rl.Rectangle{X: r.X + edge, Y: r.Y + p.px(head) + edge, Width: r.Width - 2*edge, Height: p.px(row) - edge}
		rl.DrawRectangleRec(blank, ledgerPaper)
		p.textIn(empty, inset(blank, p.px(10), 0), 13, regular, ledgerMuted, left)
	}
}

func drawLedgerAccount(p painter, r rl.Rectangle, c *contracts) {
	p.textIn("ACCOUNT TOTALS • MARKS", rl.Rectangle{X: r.X, Y: r.Y, Width: r.Width, Height: p.px(20)}, 10, semibold, ledgerMuted, left)
	for i, field := range []struct {
		label  string
		amount int
		ink    rl.Color
	}{{"Balance", c.balance, ledgerInk}, {"Debt owed", c.debt, ledgerDebit}} {
		x := r.X + float32(i)*r.Width/2
		box := rl.Rectangle{X: x, Y: r.Y + p.px(29), Width: r.Width / 2, Height: p.px(62)}
		p.textIn(field.label, inset(rl.Rectangle{X: box.X, Y: box.Y, Width: box.Width, Height: p.px(20)}, p.px(12), 0), 11, regular, ledgerMuted, left)
		p.textIn(marks(field.amount), inset(rl.Rectangle{X: box.X, Y: box.Y + p.px(23), Width: box.Width, Height: p.px(34)}, p.px(12), 0), 26, semibold, field.ink, right)
	}
	edge := max(1, p.px(1))
	for _, x := range []float32{r.X, r.X + r.Width/2, r.X + r.Width - edge} {
		rl.DrawRectangleRec(rl.Rectangle{X: x, Y: r.Y + p.px(29), Width: edge, Height: p.px(62)}, ledgerRule)
	}
	for _, y := range []float32{r.Y + p.px(29), r.Y + p.px(91)} {
		rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: y, Width: r.Width, Height: edge}, ledgerRule)
	}
}

func ledgerLine(p painter, r rl.Rectangle, text string, offset, size float32, w weight, ink rl.Color) {
	p.textIn(text, rl.Rectangle{X: r.X + p.px(12), Y: r.Y + p.px(offset), Width: r.Width - p.px(24), Height: p.px(size + 6)}, size, w, ink, left)
}

func drawContractJournal(p painter, l layout, c *contracts, focus, offset int) {
	// A cloth cover, two paper leaves, a shaded fold and the book's red margins.
	rl.DrawRectangleRec(inset(l.panel, -p.px(5), -p.px(5)), rl.NewColor(41, 35, 27, 255))
	rl.DrawRectangleRec(l.panel, ledgerPaper)
	middle := l.panel.X + l.panel.Width/2
	rl.DrawRectangleRec(rl.Rectangle{X: middle - p.px(6), Y: l.panel.Y, Width: p.px(12), Height: l.panel.Height}, rl.NewColor(103, 91, 65, 25))
	for _, x := range []float32{l.debts.X - p.px(8), l.rows[1].X - p.px(8)} {
		rl.DrawRectangleRec(rl.Rectangle{X: x, Y: l.panel.Y + p.px(150), Width: max(1, p.px(1)), Height: p.px(395)}, rl.NewColor(151, 82, 66, 95))
	}
	x, y := l.heading.X, l.heading.Y
	p.text("PERSONAL LEDGER • UNLISTED", rl.Vector2{X: x, Y: y}, 10, semibold, ledgerMuted)
	p.text("Account book", rl.Vector2{X: x, Y: y + p.px(27)}, 32, black, ledgerInk)
	p.text("Debts owed and work on record.", rl.Vector2{X: x, Y: y + p.px(78)}, 14, regular, ledgerMuted)
	drawLedgerAccount(p, l.account, c)

	section := func(name, count string, r rl.Rectangle) {
		p.text(name, rl.Vector2{X: r.X, Y: r.Y - p.px(32)}, 18, semibold, ledgerInk)
		p.textIn(count, rl.Rectangle{X: r.X + r.Width - p.px(160), Y: r.Y - p.px(34), Width: p.px(160), Height: p.px(26)}, 11, regular, ledgerMuted, right)
	}
	section("Debts owed", "Marks", l.debts)
	debtColumns := []ledgerColumn{{"CREDITOR / DEBT", .40, left}, {"ORIGINAL", .20, right}, {"REPAID", .18, right}, {"DUE", .22, right}}
	repaid := max(0, passageDebt-c.debt)
	ledgerTable(p, l.debts, debtColumns, [][]ledgerCell{{
		{text: "Ada Vellér", note: "Passage on Patience"},
		{text: marks(passageDebt)}, {text: marks(repaid), ink: ledgerCredit}, {text: marks(c.debt), ink: ledgerDebit},
	}}, []ledgerCell{{text: "Total"}, {text: marks(passageDebt)}, {text: marks(repaid), ink: ledgerCredit}, {text: marks(c.debt), ink: ledgerDebit}}, "")
	p.textIn("Work credits repay the passage debt.", rl.Rectangle{X: l.debts.X, Y: l.debts.Y + l.debts.Height + p.px(12), Width: l.debts.Width, Height: p.px(22)}, 12, regular, ledgerMuted, left)

	ongoing := l.rows[0]
	count := "0 / 1 active"
	if c.ongoing != nil {
		count = "1 / 1 active"
	}
	section("Ongoing contract", count, ongoing)
	rl.DrawRectangleRec(ongoing, ledgerHead)
	if order := c.ongoing; order != nil {
		ledgerLine(p, ongoing, order.title, 9, 18, semibold, ledgerInk)
		ledgerLine(p, ongoing, order.id+" • "+order.poster, 35, 11, regular, ledgerMuted)
		ledgerLine(p, ongoing, order.objective, 56, 13, regular, ledgerInk)
		ledgerLine(p, ongoing, fmt.Sprintf("Pay: %s marks to debt • E to hand over", marks(order.pay)), 86, 12, semibold, ledgerCredit)
	} else {
		ledgerLine(p, ongoing, "No ongoing contract", 22, 17, semibold, ledgerInk)
		hint := "One contract at a time."
		if c.state == contractAvailable {
			hint = "Ada has work at the arrivals terminal on the Pads."
		}
		ledgerLine(p, ongoing, hint, 57, 12, regular, ledgerMuted)
	}

	history := l.rows[1]
	count = fmt.Sprintf("%d receipts", len(c.completed))
	if len(c.completed) == 1 {
		count = "1 receipt"
	}
	start := min(max(0, offset), max(0, len(c.completed)-ledgerHistoryRows))
	visible := min(ledgerHistoryRows, len(c.completed)-start)
	if start > 0 || len(c.completed) > ledgerHistoryRows {
		count = fmt.Sprintf("%d-%d of %d", start+1, start+visible, len(c.completed))
	}
	section("Completed contracts", count, history)
	columns := []ledgerColumn{{"REF.", .15, left}, {"CONTRACT / PAYER", .43, left}, {"DEBT CREDIT", .23, right}, {"STATUS", .19, left}}
	var rows [][]ledgerCell
	for i := 0; i < visible; i++ {
		receipt := c.completed[len(c.completed)-1-start-i]
		rows = append(rows, []ledgerCell{{text: receipt.id}, {text: receipt.title, note: receipt.poster}, {text: marks(receipt.debtCredit), ink: ledgerCredit}, {text: "Delivered", ink: ledgerCredit}})
	}
	total := 0
	for _, receipt := range c.completed {
		total += receipt.debtCredit
	}
	ledgerTable(p, history, columns, rows, []ledgerCell{{}, {text: "Total credited"}, {text: marks(total), ink: ledgerCredit}, {}}, "No completed contracts yet.")
	p.textIn("Receipt credits reduce debt; they do not add cash.", rl.Rectangle{X: history.X, Y: history.Y + history.Height + p.px(12), Width: history.Width, Height: p.px(22)}, 12, regular, ledgerMuted, left)

	hint := "All amounts in marks"
	if len(c.completed) > ledgerHistoryRows {
		hint += " • Scroll receipts to browse"
	}
	p.textIn(hint, l.hint, 12, regular, ledgerMuted, left)
	for i, b := range l.buttons {
		fill, ink := ledgerHead, ledgerInk
		if i == focus {
			fill, ink = ledgerInk, ledgerPaper
		}
		rl.DrawRectangleRec(b, fill)
		p.textIn(items(contractJournal)[i].label, inset(b, p.px(20), 0), 17, semibold, ink, left)
	}
}
