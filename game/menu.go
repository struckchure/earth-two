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

// The menus: the game opens on the title screen, Esc pauses it, M opens
// the map (see maps.go), and the wardrobe is in both menus. Screens stack, so Esc (or Back) returns to
// whichever screen opened the one in front. Up/Down (W/S) move between
// choices and Enter picks one, or point and click.

// screen is what's in front of the game.
type screen uint8

const (
	playing screen = iota
	title
	paused
	dressing
	controlsHelp
	mapping // the full map (see maps.go)
	contractOffer
	contractJournal
	identityScreen
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
	actAcceptContract
	actIdentity
	actIdentityCreate
	actIdentityUnlock
	actIdentityConnect
	actIdentityImport
	actIdentityExport
	actIdentityEmail
	actIdentityLock
	actIdentityName
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
		out = []item{{"Play", actPlay}, {"Wardrobe", actWardrobe}, {"Controls", actControls}, {"Identity", actIdentity}}
	case paused:
		out = []item{{"Resume", actResume}, {"Wardrobe", actWardrobe}, {"Controls", actControls}, {"Main menu", actMainMenu}, {"Identity", actIdentity}}
	case controlsHelp:
		return []item{{"Back", actBack}}
	case dressing:
		return []item{{"Done", actBack}}
	case contractOffer:
		return []item{{"Accept contract", actAcceptContract}, {"Leave it for now", actBack}}
	case contractJournal:
		return []item{{"Back", actBack}}
	case identityScreen:
		return []item{{"Create identity", actIdentityCreate}, {"Unlock key", actIdentityUnlock}, {"Connect account", actIdentityConnect}, {"Lock key", actIdentityLock}, {"Import key", actIdentityImport}, {"Export key", actIdentityExport}, {"Save email", actIdentityEmail}, {"Save display name", actIdentityName}, {"Back", actBack}}
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
	since, orbit   float32
	acceptContract bool // consumed by contractChoice
	// stamp is the stamp come down to confirm the last thing done, while
	// it shows (paper.go).
	stamp    *stampMark
	identity *identityPanel
	cursor   rl.Texture2D
}

type page struct {
	screen  screen
	focus   int
	receipt int // journal history offset, newest first
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
	if m.screen() == identityScreen {
		n += identityFields
	}
	return n
}

// do carries out a. It returns true to quit the game.
func (m *menu) do(a action) bool {
	switch a {
	case actPlay:
		m.stampOn("ADMITTED", stampBlue, m.screen())
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
	case actAcceptContract:
		if m.screen() == contractOffer {
			m.acceptContract = true
			m.stampOn("FILED", stampRed, contractOffer)
			m.back()
		}
	case actIdentity:
		if m.identity == nil {
			m.identity = newIdentityPanel()
		}
		m.open(identityScreen)
	case actIdentityCreate, actIdentityUnlock, actIdentityConnect, actIdentityImport, actIdentityExport, actIdentityEmail, actIdentityLock, actIdentityName:
		if m.identity != nil {
			m.identity.act(a)
		}
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
	if p.screen == identityScreen {
		if p.focus < identityFields {
			p.focus = (p.focus + 1) % identityFields
			return false
		}
		return m.do(items(identityScreen)[p.focus-identityFields].act)
	}
	return m.do(items(p.screen)[p.focus].act)
}

// nav is a frame's menu keys.
type nav struct {
	up, down, left, right, enter, back, mapKey, journalKey bool
}

func readNav(k *input.Keys) nav {
	return nav{
		up:    k.AnyJustPressed(rl.KeyUp, rl.KeyW),
		down:  k.AnyJustPressed(rl.KeyDown, rl.KeyS),
		left:  k.AnyJustPressed(rl.KeyLeft, rl.KeyA),
		right: k.AnyJustPressed(rl.KeyRight, rl.KeyD),
		enter: k.AnyJustPressed(rl.KeyEnter, rl.KeyKpEnter, rl.KeySpace),
		back:  k.AnyJustPressed(rl.KeyEscape, rl.KeyBackspace),
		// mapKey opens the full map in play and closes it.
		mapKey:     k.JustPressed(rl.KeyM),
		journalKey: k.JustPressed(rl.KeyJ),
	}
}

// navigate applies a frame's keys. It returns true to quit the game.
func (m *menu) navigate(n nav, w *character.Wardrobe, o *character.Outfit) bool {
	s := m.screen()
	switch {
	case s == playing && n.back:
		m.open(paused)
		return false
	case s == playing && n.mapKey:
		m.open(mapping)
		return false
	case s == playing && n.journalKey:
		m.open(contractJournal)
		return false
	case s == playing:
		return false
	case s == mapping:
		// It has no choices: M or Esc closes it.
		if n.mapKey || n.back {
			m.back()
		}
		return false
	case s == contractJournal && n.journalKey:
		m.back()
		return false
	case n.back:
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
		case p.screen == identityScreen && h.focus < identityFields:
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
	headingH = 76
	buttonH  = 50
	gap      = 10
	rowH     = 46
	arrow    = 34
	bindingH = 33
)

// layout is where one screen's parts are, in pixels at scale sc.
type layout struct {
	sc float32
	// panel is the screen's panel, or on the title screen its column.
	panel   rl.Rectangle
	heading rl.Vector2
	buttons []rl.Rectangle
	// rows are wardrobe rows, control bindings or the journal's record cards.
	rows, left, right []rl.Rectangle
	hint              rl.Rectangle
	account           rl.Rectangle
	debts             rl.Rectangle
	available         rl.Rectangle
}

// down is l moved dy down the screen (a filed form taken away: paper.go).
func (l layout) down(dy float32) layout {
	move := func(rs []rl.Rectangle) []rl.Rectangle {
		out := make([]rl.Rectangle, len(rs))
		for i, r := range rs {
			r.Y += dy
			out[i] = r
		}
		return out
	}
	l.panel.Y += dy
	l.heading.Y += dy
	l.buttons, l.rows, l.left, l.right = move(l.buttons), move(l.rows), move(l.left), move(l.right)
	for _, r := range []*rl.Rectangle{&l.hint, &l.account, &l.debts, &l.available} {
		r.Y += dy
	}
	return l
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
		// The arrival form: its header and a typed line, the choices, and
		// room under them for the Registrar's seal.
		const x, w, head, foot = 80, 340, 128, 78
		h := head + stack(n) + foot
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(x), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(x), Y: pt(y)}
		buttons(x, y+head, w)
	case paused:
		const w = 340
		h := pad + headingH + stack(n) + pad
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(margin), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(margin + pad), Y: pt(y + pad)}
		buttons(margin+pad, y+pad+headingH, w-2*pad)
	case controlsHelp:
		const w = 500
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
		// The rows as tall as the window has room for, down to compact.
		row := float32(rowH)
		if room := height/sc - 48 - (pad + headingH + gap + stack(n) + pad); room < row*float32(rowCount) {
			row = max(34, room/float32(rowCount))
		}
		box := min(arrow, row-8)
		rows := float32(rowCount) * row
		h := pad + headingH + rows + gap + stack(n) + pad
		y := centreY(h)
		l.panel = rl.Rectangle{X: pt(margin), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(margin + pad), Y: pt(y + pad)}
		var x, rw float32 = margin + pad - 10, w - 2*pad + 20
		for i := range rowCount {
			ry := y + pad + headingH + float32(i)*row
			ay := ry + (row-box)/2
			l.rows = append(l.rows, rl.Rectangle{X: pt(x), Y: pt(ry), Width: pt(rw), Height: pt(row - 4)})
			l.left = append(l.left, rl.Rectangle{X: pt(x + 118), Y: pt(ay - 2), Width: pt(box), Height: pt(box)})
			l.right = append(l.right, rl.Rectangle{X: pt(x + rw - 6 - box), Y: pt(ay - 2), Width: pt(box), Height: pt(box)})
		}
		by := y + pad + headingH + rows + gap
		buttons(margin+pad, by, w-2*pad)
	case contractOffer:
		const w, h = 540, 580
		x, y := max(16, (width/sc-w)/2), centreY(h)
		l.panel = rl.Rectangle{X: pt(x), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(x + pad), Y: pt(y + pad)}
		buttons(x+pad, y+h-pad-stack(n), w-2*pad)
	case contractJournal:
		const w, h = 1040, 680
		// The journal fits even when a window is smaller than the normal
		// minimum UI scale. Drawing uses this same scale as the mouse hits.
		sc = min(sc, width/(w+32), height/(h+32))
		l.sc = sc
		x, y := (width/sc-w)/2, (height/sc-h)/2
		l.panel = rl.Rectangle{X: pt(x), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.heading = rl.Vector2{X: pt(x + pad), Y: pt(y + pad)}
		const pageW = w/2 - 2*pad
		l.account = rl.Rectangle{X: pt(x + w/2 + pad), Y: pt(y + pad), Width: pt(pageW), Height: pt(108)}
		l.debts = rl.Rectangle{X: pt(x + pad), Y: pt(y + 174), Width: pt(pageW), Height: pt(110)}
		l.available = rl.Rectangle{X: pt(x + pad), Y: pt(y + 358), Width: pt(pageW), Height: pt(80)}
		l.rows = []rl.Rectangle{
			{X: pt(x + pad), Y: pt(y + 514), Width: pt(pageW), Height: pt(96)},
			{X: pt(x + w/2 + pad), Y: pt(y + 194), Width: pt(pageW), Height: pt(28 + ledgerHistoryRows*52 + 30)},
		}
		l.hint = rl.Rectangle{X: pt(x + pad), Y: pt(y + h - pad - buttonH), Width: pt(w - 2*pad - 200), Height: pt(buttonH)}
		buttons(x+w-pad-180, y+h-pad-buttonH, 180)
	case identityScreen:
		const w, h = 760, 760
		sc = min(sc, width/(w+32), height/(h+32))
		l.sc = sc
		x, y := (width/sc-w)/2, (height/sc-h)/2
		l.panel = rl.Rectangle{X: pt(x), Y: pt(y), Width: pt(w), Height: pt(h)}
		l.account = rl.Rectangle{X: pt(x + pad), Y: pt(y + 126), Width: pt(w - 2*pad), Height: pt(30)}
		for i := range identityFields {
			l.rows = append(l.rows, rl.Rectangle{X: pt(x + pad), Y: pt(y + 170 + float32(i)*54), Width: pt(w - 2*pad), Height: pt(48)})
		}
		for i := range n {
			r := rl.Rectangle{X: pt(x + pad + float32(i%2)*(w/2-pad)), Y: pt(y + 408 + float32(i/2)*48), Width: pt(w/2 - pad - 10), Height: pt(42)}
			if i == n-1 && n%2 == 1 {
				r.Width = pt(w - 2*pad)
			}
			l.buttons = append(l.buttons, r)
		}
		l.hint = rl.Rectangle{X: pt(x + pad), Y: pt(y + 668), Width: pt(w - 2*pad), Height: pt(28)}
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
	if s == identityScreen {
		for i, r := range l.rows {
			out = append(out, hit{r, i, 0})
		}
		first = identityFields
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
	if mu.identity != nil {
		mu.identity.poll()
	}
	navigation := readNav(keys.Get())
	if mu.screen() == identityScreen {
		navigation = nav{up: keys.Get().JustPressed(rl.KeyUp), down: keys.Get().JustPressed(rl.KeyDown), enter: keys.Get().JustPressed(rl.KeyEnter), back: keys.Get().JustPressed(rl.KeyEscape)}
		mu.identity.edit(mu.top().focus)
		if keys.Get().JustPressed(rl.KeyTab) {
			mu.top().focus = wrap(mu.top().focus+1, mu.choices())
		}
	}
	identityClipboardFocus(mu.screen() == identityScreen && mu.top().focus < identityFields && !mu.identity.busy)
	quit := mu.navigate(navigation, w, o)
	if mu.screen() == playing && buttons.Get().JustPressed(rl.MouseButtonLeft) {
		p := painter{s: uiScale(ww)}
		card := accountSummaryRect(p, float32(ww.Width))
		if contains(card, mouse.Get().Position) || contains(accountBadgeRect(p, card), mouse.Get().Position) {
			mu.open(contractJournal)
		}
	}
	if s := mu.screen(); s != playing && !quit {
		ms := mouse.Get()
		moved := ms.Delta.X != 0 || ms.Delta.Y != 0
		l := layoutFor(s, float32(ww.Width), float32(ww.Height), uiScale(ww))
		if s == contractJournal && contains(l.rows[1], ms.Position) {
			if ms.Wheel < 0 {
				mu.top().receipt++
			}
			if ms.Wheel > 0 {
				mu.top().receipt = max(0, mu.top().receipt-1)
			}
		}
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
