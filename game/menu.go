package game

import (
	"runtime"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/window"
)

// The menus: the game opens on the title screen, Esc pauses it, and C opens
// the wardrobe. Screens stack, so Esc (or Back) returns to whichever screen
// opened the one in front. Up/Down (W/S) move between choices and Enter
// picks one, or point and click.

// screen is what's in front of the game.
type screen uint8

const (
	playing screen = iota
	title
	paused
	dressing
	controlsHelp
)

type action uint8

const (
	actPlay action = iota
	actResume
	actWardrobe
	actControls
	actMainMenu
	actQuit
	actBack
)

type item struct {
	label string
	act   action
}

// canQuit is whether the game can close itself: a browser tab can't.
const canQuit = runtime.GOOS != "js"

// items are a screen's buttons, top to bottom.
func items(s screen) []item {
	var out []item
	switch s {
	case title:
		out = []item{{"Play", actPlay}, {"Wardrobe", actWardrobe}, {"Controls", actControls}}
	case paused:
		out = []item{{"Resume", actResume}, {"Wardrobe", actWardrobe}, {"Controls", actControls}, {"Main menu", actMainMenu}}
	case controlsHelp:
		return []item{{"Back", actBack}}
	case dressing:
		return []item{{"Done", actBack}}
	default:
		return nil
	}
	if canQuit {
		out = append(out, item{"Quit", actQuit})
	}
	return out
}

// menu is a resource: the screens in front of the game, the last on top,
// each with its focused choice. No screens is playing.
type menu struct {
	stack []page
	// since is how long the top screen has been up, and orbit how far the
	// title screen's camera has turned.
	since, orbit float32
}

type page struct {
	screen screen
	focus  int
}

func newMenu() *menu { return &menu{stack: []page{{screen: title}}} }

func (m *menu) screen() screen {
	if len(m.stack) == 0 {
		return playing
	}
	return m.stack[len(m.stack)-1].screen
}

func (m *menu) top() *page { return &m.stack[len(m.stack)-1] }

func (m *menu) open(s screen) {
	m.stack = append(m.stack, page{screen: s})
	m.since = 0
}

// back closes the screen in front, unless it's the title screen.
func (m *menu) back() {
	if len(m.stack) == 0 || len(m.stack) == 1 && m.stack[0].screen == title {
		return
	}
	m.stack = m.stack[:len(m.stack)-1]
	m.since = 0
}

// onTitle is whether the title screen is under the one in front.
func (m *menu) onTitle() bool { return len(m.stack) > 0 && m.stack[0].screen == title }

// choices is how many things the top screen can focus: the wardrobe's rows
// and its button, or the screen's buttons.
func (m *menu) choices() int {
	n := len(items(m.screen()))
	if m.screen() == dressing {
		n += rowCount
	}
	return n
}

// do carries out a. It returns true to quit the game.
func (m *menu) do(a action) bool {
	switch a {
	case actPlay:
		m.stack, m.since = nil, 0
	case actResume, actBack:
		m.back()
	case actWardrobe:
		m.open(dressing)
	case actControls:
		m.open(controlsHelp)
	case actMainMenu:
		m.stack, m.since = []page{{screen: title}}, 0
	case actQuit:
		return canQuit
	}
	return false
}

// press picks the focused choice: a button, or on the wardrobe's rows, the
// next thing to wear.
func (m *menu) press(w *character.Wardrobe, o *character.Outfit) bool {
	p := m.top()
	if p.screen == dressing && p.focus < rowCount {
		*o = cycleRow(w, *o, p.focus, 1)
		return false
	}
	if p.screen == dressing {
		return m.do(items(dressing)[p.focus-rowCount].act)
	}
	return m.do(items(p.screen)[p.focus].act)
}

// nav is a frame's menu keys.
type nav struct {
	up, down, left, right, enter, back, wardrobe bool
}

func readNav(k *input.Keys) nav {
	return nav{
		up:       k.AnyJustPressed(rl.KeyUp, rl.KeyW),
		down:     k.AnyJustPressed(rl.KeyDown, rl.KeyS),
		left:     k.AnyJustPressed(rl.KeyLeft, rl.KeyA),
		right:    k.AnyJustPressed(rl.KeyRight, rl.KeyD),
		enter:    k.AnyJustPressed(rl.KeyEnter, rl.KeyKpEnter, rl.KeySpace),
		back:     k.AnyJustPressed(rl.KeyEscape, rl.KeyBackspace),
		wardrobe: k.JustPressed(rl.KeyC),
	}
}

// navigate applies a frame's keys. It returns true to quit the game.
func (m *menu) navigate(n nav, w *character.Wardrobe, o *character.Outfit) bool {
	s := m.screen()
	switch {
	case s == playing && n.back:
		m.open(paused)
		return false
	case s == playing && n.wardrobe:
		m.open(dressing)
		return false
	case s == playing:
		return false
	case n.back, s == dressing && n.wardrobe:
		m.back()
		return false
	}
	p := m.top()
	switch {
	case n.up:
		p.focus = wrap(p.focus-1, m.choices())
	case n.down:
		p.focus = wrap(p.focus+1, m.choices())
	case (n.left || n.right) && s == dressing && p.focus < rowCount:
		step := 1
		if n.left {
			step = -1
		}
		*o = cycleRow(w, *o, p.focus, step)
	case n.enter:
		return m.press(w, o)
	}
	return false
}

// hit is a place to click: it focuses a choice, and on the wardrobe's
// arrows changes it by step.
type hit struct {
	rect  rl.Rectangle
	focus int
	step  int
}

// point applies the mouse: moving over a choice focuses it, and clicking
// picks it. It returns true to quit the game.
func (m *menu) point(hits []hit, pos rl.Vector2, moved, clicked bool, w *character.Wardrobe, o *character.Outfit) bool {
	if m.screen() == playing {
		return false
	}
	for _, h := range hits {
		if !contains(h.rect, pos) {
			continue
		}
		p := m.top()
		if moved || clicked {
			p.focus = h.focus
		}
		switch {
		case !clicked:
		case h.step != 0:
			*o = cycleRow(w, *o, h.focus, h.step)
		case p.screen == dressing && h.focus < rowCount:
			// Clicking a row's name just picks the row.
		default:
			return m.press(w, o)
		}
		return false
	}
	return false
}

// Sizes, in points.
const (
	margin   = 48
	pad      = 28
	headingH = 64
	buttonH  = 50
	gap      = 10
	rowH     = 46
	arrow    = 34
	bindingH = 40
)

// layout is where one screen's parts are, in pixels at scale sc.
type layout struct {
	sc float32
	// panel is the screen's panel, or on the title screen its column.
	panel   rl.Rectangle
	heading rl.Vector2
	buttons []rl.Rectangle
	// rows are the wardrobe's rows (with their arrows) or the controls.
	rows, left, right []rl.Rectangle
	hint              rl.Rectangle
}

// layoutFor lays out s in a width by height window.
func layoutFor(s screen, width, height, sc float32) layout {
	l := layout{sc: sc}
	pt := func(v float32) float32 { return v * sc }
	n := len(items(s))
	stack := func(n int) float32 { return float32(n)*(buttonH+gap) - gap }
	centreY := func(h float32) float32 { return max(24, (height/sc-h)/2) }
	buttons := func(x, y, w float32) {
		for i := range n {
			l.buttons = append(l.buttons, rl.Rectangle{X: pt(x), Y: pt(y + float32(i)*(buttonH+gap)), Width: pt(w), Height: pt(buttonH)})
		}
	}
	switch s {
	case title:
		const x, w = 80, 320
		h := 110 + stack(n)
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(x), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(x), Y: pt(y)}
		buttons(x, y+110, w)
	case paused:
		const w = 340
		h := pad + headingH + stack(n) + pad
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(margin), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(margin + pad), Y: pt(y + pad)}
		buttons(margin+pad, y+pad+headingH, w-2*pad)
	case controlsHelp:
		const w = 420
		h := pad + headingH + float32(len(bindings))*bindingH + 2*gap + stack(n) + pad
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(margin), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(margin + pad), Y: pt(y + pad)}
		for i := range bindings {
			l.rows = append(l.rows, rl.Rectangle{X: pt(margin + pad), Y: pt(y + pad + headingH + float32(i)*bindingH), Width: pt(w - 2*pad), Height: pt(bindingH)})
		}
		buttons(margin+pad, y+pad+headingH+float32(len(bindings))*bindingH+2*gap, w-2*pad)
	case dressing:
		const w = 400
		rows := float32(rowCount) * rowH
		h := pad + headingH + rows + gap + stack(n) + 38 + pad
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(margin), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(margin + pad), Y: pt(y + pad)}
		var x, rw float32 = margin + pad - 10, w - 2*pad + 20
		for i := range rowCount {
			ry := y + pad + headingH + float32(i)*rowH
			ay := ry + (rowH-arrow)/2
			l.rows = append(l.rows, rl.Rectangle{X: pt(x), Y: pt(ry), Width: pt(rw), Height: pt(rowH - 4)})
			l.left = append(l.left, rl.Rectangle{X: pt(x + 118), Y: pt(ay - 2), Width: pt(arrow), Height: pt(arrow)})
			l.right = append(l.right, rl.Rectangle{X: pt(x + rw - 6 - arrow), Y: pt(ay - 2), Width: pt(arrow), Height: pt(arrow)})
		}
		by := y + pad + headingH + rows + gap
		buttons(margin+pad, by, w-2*pad)
		l.hint = rl.Rectangle{X: pt(margin + pad), Y: pt(by + stack(n) + 10), Width: pt(w - 2*pad), Height: pt(28)}
	}
	return l
}

// hits are the layout's places to click on screen s.
func (l layout) hits(s screen) []hit {
	var out []hit
	first := 0
	if s == dressing {
		for i := range l.rows {
			out = append(out, hit{l.left[i], i, -1}, hit{l.right[i], i, 1}, hit{l.rows[i], i, 0})
		}
		first = rowCount
	}
	for i, b := range l.buttons {
		out = append(out, hit{b, first + i, 0})
	}
	return out
}

// menuInput runs the menus from the keyboard and mouse.
func menuInput(
	keys *illusion.Res[input.Keys],
	buttons *illusion.Res[input.MouseButtons],
	mouse *illusion.Res[input.Mouse],
	m *illusion.Res[menu],
	players *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
	wardrobe *illusion.Res[character.Wardrobe],
	win *illusion.Res[window.Window],
	exit *illusion.Res[illusion.AppExit],
) {
	_, o, ok := players.Single()
	if !ok {
		return
	}
	mu, w, ww := m.Get(), wardrobe.Get(), win.Get()
	quit := mu.navigate(readNav(keys.Get()), w, o)
	if s := mu.screen(); s != playing && !quit {
		ms := mouse.Get()
		moved := ms.Delta.X != 0 || ms.Delta.Y != 0
		l := layoutFor(s, float32(ww.Width), float32(ww.Height), uiScale(ww))
		quit = mu.point(l.hits(s), ms.Position, moved, buttons.Get().JustPressed(rl.MouseButtonLeft), w, o)
	}
	if quit {
		exit.Get().Requested = true
	}
}

// lockControls stops the player while a menu is up. It runs after
// menuInput, which runs after the player's input: a menu that opens stops
// the player from the next frame, and the key that starts play (Enter or
// Space) has been and gone by the time the player's input reads it.
func lockControls(m *illusion.Res[menu], controls *illusion.Res[character.Controls], t *illusion.Res[illusion.Time], settings *illusion.Res[physics.Settings]) {
	mu := m.Get()
	controls.Get().Enabled = mu.screen() == playing
	settings.Get().Paused = !controls.Get().Enabled
	dt := t.Get().DeltaSecs()
	mu.since += dt
	if mu.onTitle() {
		mu.orbit += dt
	}
}
