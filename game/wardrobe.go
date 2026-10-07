package game

import (
	"strings"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/transform"
)

// The wardrobe changes what the player looks like: the body, the skin tone
// and what they wear in each slot. Pick it from the title screen or the
// pause menu (see menu.go). The player turns to the camera, which swings round to
// their front; Up/Down pick a row and Left/Right change it, or click its
// arrows.

// Rows: the body, the skin tone, a faction's look, then one per
// character.Slot.
const (
	rowBody = iota
	rowTone
	rowLook
	rowSlots
	rowCount = rowSlots + int(character.SlotCount)
)

func wrap(i, n int) int { return ((i % n) + n) % n }

// cycleRow moves row's choice on o by step, wrapping round. Slots cycle
// through nothing and then their items.
func cycleRow(w *character.Wardrobe, o character.Outfit, row, step int) character.Outfit {
	switch row {
	case rowBody:
		if n := len(w.Bodies); n > 1 {
			return w.Rebody(o, wrap(o.Body+step, n))
		}
		return o
	case rowTone:
		if n := len(w.Bodies[o.Body].Tones); n > 0 {
			o.Tone = wrap(o.Tone+step, n)
		}
		return o
	case rowLook:
		// Looks cycle on from whichever is worn, or from the first.
		n := len(w.Bodies[o.Body].Looks)
		if n == 0 {
			return o
		}
		next := 0
		if i, ok := w.LookOf(o); ok {
			next = wrap(i+step, n)
		} else if step < 0 {
			next = n - 1
		}
		return w.Wear(o, next)
	}
	slot := character.Slot(row - rowSlots)
	n := len(w.Bodies[o.Body].Items[slot])
	if n == 0 {
		return o
	}
	current := 0
	if i, ok := o.Item(slot); ok {
		current = i + 1
	}
	if next := wrap(current+step, n+1); next == 0 {
		o.Remove(slot)
	} else {
		o.Put(slot, next-1)
	}
	return o
}

// rowText is a row's label and its current choice.
func rowText(w *character.Wardrobe, o character.Outfit, row int) (label, value string) {
	body := &w.Bodies[o.Body]
	switch row {
	case rowBody:
		return "Body", capitalise(body.Name)
	case rowTone:
		if o.Tone < len(body.Tones) {
			return "Skin", body.Tones[o.Tone].Name
		}
		return "Skin", "-"
	case rowLook:
		if i, ok := w.LookOf(o); ok {
			return "Look", body.Looks[i].Name
		}
		return "Look", "Own"
	}
	slot := character.Slot(row - rowSlots)
	if i, ok := o.Item(slot); ok && i < len(body.Items[slot]) {
		return slot.String(), body.Items[slot][i].Name
	}
	if slot == character.Face {
		// The face the body comes with.
		return slot.String(), "Standard"
	}
	return slot.String(), "None"
}

func capitalise(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

// faceCamera turns the player's body to face +Z, where the camera is for
// the wardrobe and the title screen.
func faceCamera(
	m *illusion.Res[menu],
	bodies *illusion.Query1Where[transform.Transform, illusion.With[character.Body]],
	players *illusion.Query0Where[illusion.With[character.Player]],
	hier *illusion.Hierarchy,
	t *illusion.Res[illusion.Time],
) {
	if mu := m.Get(); mu.screen() != dressing && !mu.onTitle() {
		return
	}
	k := min(1, 8*t.Get().DeltaSecs())
	bodies.Each(func(e ecs.Entity, tr *transform.Transform) {
		if parent, ok := hier.Parent(e); ok && players.Contains(parent) {
			tr.Rotation = rl.QuaternionSlerp(tr.Rotation, rl.QuaternionIdentity(), k)
		}
	})
}

// drawWardrobe draws the wardrobe as a form (paper.go), like the menus: a
// row for each choice, its label in small capitals over the choice typed
// between arrows in ink, the row in hand gone over with a highlighter; and
// the menu's choices as boxes to tick.
func drawWardrobe(p painter, l layout, focus int, w *character.Wardrobe, o character.Outfit) {
	p.paper(inset(l.panel, -p.px(10), -p.px(4)))
	header := rl.Rectangle{X: l.heading.X, Y: l.heading.Y, Width: l.panel.X + l.panel.Width - p.px(pad) - l.heading.X, Height: p.px(headingH)}
	p.formHeader(header, "THE EXCHANGE  \u2022  LANDFALL", "FORM W-1", "Wardrobe", 24)
	rule := max(1, p.px(1))
	for i, r := range l.rows {
		label, value := rowText(w, o, i)
		between := rl.Rectangle{X: l.left[i].X + l.left[i].Width + p.px(6), Y: r.Y, Height: r.Height}
		between.Width = l.right[i].X - p.px(6) - between.X
		if i == focus {
			rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y + r.Height*.18, Width: r.Width, Height: r.Height * .64}, highlighter)
		}
		p.textIn(strings.ToUpper(label), rl.Rectangle{X: r.X + p.px(6), Y: r.Y, Width: l.left[i].X - r.X - p.px(10), Height: r.Height}, 11, semibold, ledgerMuted, left)
		for _, a := range []struct {
			r   rl.Rectangle
			dir float32
		}{{l.left[i], -1}, {l.right[i], 1}} {
			outline(a.r, max(1, p.px(1.5)), ledgerInk)
			p.chevron(a.r, a.dir, ledgerInk)
		}
		face := typed
		if i == focus {
			face = typedBold
		}
		p.textIn(value, between, 16, face, ledgerInk, centre)
		rl.DrawRectangleRec(rl.Rectangle{X: r.X, Y: r.Y + r.Height - rule, Width: r.Width, Height: rule}, ledgerRule)
	}
	for i, b := range l.buttons {
		p.tickChoice(b, items(dressing)[i].label, focus == rowCount+i)
	}
}
