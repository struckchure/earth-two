package game

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
)

func TestMenuStartsOnTitleAndPlays(t *testing.T) {
	m := newMenu()
	w, o := menuWardrobe(), &character.Outfit{}
	if m.screen() != title {
		t.Fatalf("starts on %v, want the title screen", m.screen())
	}
	m.navigate(nav{back: true}, w, o)
	if m.screen() != title {
		t.Errorf("Esc left the title screen for %v", m.screen())
	}
	m.navigate(nav{enter: true}, w, o) // Play
	if m.screen() != playing {
		t.Errorf("Play: on %v, want playing", m.screen())
	}
}

func TestMenuPauseAndBack(t *testing.T) {
	m := &menu{}
	w, o := menuWardrobe(), &character.Outfit{}
	m.navigate(nav{back: true}, w, o)
	if m.screen() != paused {
		t.Fatalf("Esc in play: %v, want paused", m.screen())
	}
	m.navigate(nav{down: true}, w, o)
	m.navigate(nav{down: true}, w, o)
	m.navigate(nav{enter: true}, w, o) // Controls
	if m.screen() != controlsHelp {
		t.Fatalf("third choice: %v, want the controls", m.screen())
	}
	m.navigate(nav{back: true}, w, o)
	if m.screen() != paused || m.top().focus != 2 {
		t.Errorf("back from the controls: %v focus %d, want paused on Controls", m.screen(), m.top().focus)
	}
	m.navigate(nav{back: true}, w, o)
	if m.screen() != playing {
		t.Errorf("Esc on the pause menu: %v, want playing", m.screen())
	}
}

func TestMenuMainMenuAndQuit(t *testing.T) {
	m := &menu{}
	w, o := menuWardrobe(), &character.Outfit{}
	m.open(paused)
	m.top().focus = 3 // Main menu
	m.navigate(nav{enter: true}, w, o)
	if m.screen() != title || len(m.stack) != 1 {
		t.Fatalf("Main menu: %v %v, want just the title screen", m.screen(), m.stack)
	}
	m.navigate(nav{up: true}, w, o) // wraps to the last: Quit
	if quit := m.navigate(nav{enter: true}, w, o); quit != canQuit {
		t.Errorf("Quit: %v, want %v", quit, canQuit)
	}
}

func TestMenuWardrobeKeys(t *testing.T) {
	m := &menu{}
	w, o := menuWardrobe(), &character.Outfit{}
	m.navigate(nav{wardrobe: true}, w, o)
	if m.screen() != dressing {
		t.Fatalf("C in play: %v, want the wardrobe", m.screen())
	}
	top := rowSlots + int(character.Top)
	m.top().focus = top
	m.navigate(nav{right: true}, w, o)
	if _, got := rowText(w, *o, top); got != "T-shirt" {
		t.Errorf("right on Top: %q, want T-shirt", got)
	}
	m.navigate(nav{enter: true}, w, o)
	if _, got := rowText(w, *o, top); got != "Polo" {
		t.Errorf("Enter on Top: %q, want Polo", got)
	}
	for range top + 1 {
		m.navigate(nav{up: true}, w, o) // past Body, to Done
	}
	if m.top().focus != rowCount {
		t.Fatalf("focus %d, want Done (%d)", m.top().focus, rowCount)
	}
	m.navigate(nav{enter: true}, w, o)
	if m.screen() != playing {
		t.Errorf("Done: %v, want playing", m.screen())
	}
	m.navigate(nav{wardrobe: true}, w, o)
	m.navigate(nav{wardrobe: true}, w, o)
	if m.screen() != playing {
		t.Errorf("C twice: %v, want playing", m.screen())
	}
}

func middle(r rl.Rectangle) rl.Vector2 { return rl.Vector2{X: r.X + r.Width/2, Y: r.Y + r.Height/2} }

func TestMenuMouse(t *testing.T) {
	w, o := menuWardrobe(), &character.Outfit{}
	m := newMenu()
	l := layoutFor(title, 1600, 1000, 2)
	m.point(l.hits(title), middle(l.buttons[2]), true, false, w, o)
	if m.top().focus != 2 {
		t.Errorf("hovering Controls: focus %d, want 2", m.top().focus)
	}
	m.point(l.hits(title), middle(l.buttons[1]), false, true, w, o)
	if m.screen() != dressing {
		t.Fatalf("clicking Wardrobe: %v", m.screen())
	}
	l = layoutFor(dressing, 1600, 1000, 2)
	top := rowSlots + int(character.Top)
	m.point(l.hits(dressing), middle(l.left[top]), false, true, w, o)
	if _, got := rowText(w, *o, top); got != "Polo" {
		t.Errorf("left arrow on Top: %q, want Polo", got)
	}
	m.point(l.hits(dressing), rl.Vector2{X: l.rows[1].X + 5, Y: middle(l.rows[1]).Y}, false, true, w, o)
	if m.top().focus != 1 || m.screen() != dressing {
		t.Errorf("clicking the Skin row: focus %d on %v, want 1 on the wardrobe", m.top().focus, m.screen())
	}
	m.point(l.hits(dressing), middle(l.buttons[0]), false, true, w, o)
	if m.screen() != title {
		t.Errorf("clicking Done: %v, want back on the title screen", m.screen())
	}
	if m.point(l.hits(dressing), rl.Vector2{X: 5000, Y: 5000}, true, true, w, o) {
		t.Error("a click off the menu quit")
	}
}

func TestLayoutsFitTheirPanels(t *testing.T) {
	for _, s := range []screen{title, paused, controlsHelp, dressing} {
		l := layoutFor(s, 1280, 800, 1)
		if len(l.buttons) != len(items(s)) {
			t.Errorf("%v: %d buttons for %d items", s, len(l.buttons), len(items(s)))
		}
		for _, r := range append(append([]rl.Rectangle{}, l.buttons...), l.rows...) {
			if r.X < l.panel.X || r.Y < l.panel.Y || r.X+r.Width > l.panel.X+l.panel.Width+0.01 || r.Y+r.Height > l.panel.Y+l.panel.Height+0.01 {
				t.Errorf("%v: %+v outside the panel %+v", s, r, l.panel)
			}
		}
	}
}

func TestFrame(t *testing.T) {
	eye, target := rl.Vector3{Z: 4}, rl.Vector3{}
	e, tg := frame(eye, target, 0, 45, 1.5)
	if e != eye || tg != target {
		t.Errorf("x = 0 moved the camera: %v %v", e, tg)
	}
	// Looking down -Z, the screen's right is +X; to put the target right
	// of centre the camera moves left.
	e, tg = frame(eye, target, 0.5, 45, 1.5)
	want := -0.5 * 4 * float32(math.Tan(math.Pi/8)) * 1.5
	if math.Abs(float64(e.X-want)) > 1e-4 || math.Abs(float64(tg.X-want)) > 1e-4 || e.Z != 4 {
		t.Errorf("frame: eye %v target %v, want both moved to x = %v", e, tg, want)
	}
}
