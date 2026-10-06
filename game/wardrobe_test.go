package game

import (
	"testing"

	"github.com/struckchure/earth-two/character"
)

func menuWardrobe() *character.Wardrobe {
	man := character.BodyWardrobe{Name: "man", Tones: []character.Tone{{Name: "Dark"}, {Name: "Fair"}}}
	man.Items[character.Top] = []character.Item{{Name: "T-shirt"}, {Name: "Polo"}}
	man.Items[character.OnePiece] = []character.Item{{Name: "Suit & tie"}}
	woman := character.BodyWardrobe{Name: "woman", Tones: []character.Tone{{Name: "Dark"}}}
	woman.Items[character.Top] = []character.Item{{Name: "Tank top"}, {Name: "T-shirt"}}
	return &character.Wardrobe{Bodies: []character.BodyWardrobe{man, woman}}
}

func TestCycleSlotGoesThroughNone(t *testing.T) {
	w := menuWardrobe()
	var o character.Outfit
	top := rowSlots + int(character.Top)
	want := []string{"T-shirt", "Polo", "None", "T-shirt"}
	for _, name := range want {
		o = cycleRow(w, o, top, 1)
		if _, got := rowText(w, o, top); got != name {
			t.Fatalf("cycling right: %q, want %q", got, name)
		}
	}
	o = cycleRow(w, o, top, -1)
	if _, got := rowText(w, o, top); got != "None" {
		t.Errorf("cycling left from the first item: %q, want None", got)
	}
}

func TestCycleOnePieceTakesOffTop(t *testing.T) {
	w := menuWardrobe()
	var o character.Outfit
	o = cycleRow(w, o, rowSlots+int(character.Top), 1)
	o = cycleRow(w, o, rowSlots+int(character.OnePiece), 1)
	if _, got := rowText(w, o, rowSlots+int(character.Top)); got != "None" {
		t.Errorf("top under a one-piece: %q, want None", got)
	}
}

func TestCycleBodyAndTone(t *testing.T) {
	w := menuWardrobe()
	var o character.Outfit
	o = cycleRow(w, o, rowTone, 1)
	if _, got := rowText(w, o, rowTone); got != "Fair" {
		t.Errorf("tone: %q, want Fair", got)
	}
	o = cycleRow(w, o, rowSlots+int(character.Top), 1) // T-shirt
	o = cycleRow(w, o, rowBody, 1)
	if label, got := rowText(w, o, rowBody); label != "Body" || got != "Woman" {
		t.Errorf("body: %q %q, want Body Woman", label, got)
	}
	if _, got := rowText(w, o, rowTone); got != "Dark" {
		t.Errorf("tone on a body with one: %q, want Dark", got)
	}
	if _, got := rowText(w, o, rowSlots+int(character.Top)); got != "T-shirt" {
		t.Errorf("top after the body change: %q, want her T-shirt", got)
	}
	o = cycleRow(w, o, rowBody, 1)
	if _, got := rowText(w, o, rowBody); got != "Man" {
		t.Errorf("body wraps: %q, want Man", got)
	}
}

func TestCycleLook(t *testing.T) {
	w := menuWardrobe()
	man := &w.Bodies[0]
	man.Looks = []character.Look{
		character.NewLook("Corvane", map[character.Slot]int{character.OnePiece: 0}),
		character.NewLook("Casual", map[character.Slot]int{character.Top: 1}, character.OnePiece),
	}
	var o character.Outfit
	if _, got := rowText(w, o, rowLook); got != "Own" {
		t.Fatalf("bare: %q, want Own", got)
	}
	for _, want := range []string{"Corvane", "Casual", "Corvane"} {
		o = cycleRow(w, o, rowLook, 1)
		if _, got := rowText(w, o, rowLook); got != want {
			t.Fatalf("cycling looks: %q, want %q", got, want)
		}
	}
	if _, got := rowText(w, o, rowSlots+int(character.OnePiece)); got != "Suit & tie" {
		t.Errorf("Corvane's one-piece: %q, want Suit & tie", got)
	}
	o = cycleRow(w, o, rowSlots+int(character.Top), 1)
	if _, got := rowText(w, o, rowLook); got != "Own" {
		t.Errorf("after changing the top: %q, want Own", got)
	}
}
