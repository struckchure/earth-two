package game

import (
	"cmp"
	"fmt"
	"math"
	"slices"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// The maps: a minimap in the top left corner in play, a compass along the top, and
// the full map, which M opens and closes (as a screen: it pauses, like the
// menus). Both maps are drawn from the layout, each piece as the footprint
// of its colliders, coloured by the part of the world it's from; north
// (the game's -Z) is up. They don't turn with the camera: the browser
// build's raylib can't clip drawing to the minimap's frame, but a
// rectangle that doesn't turn can be cut to it exactly.

// Sizes, in points, and how much of the world the minimap shows.
const (
	minimapSize  = 150
	minimapRange = 110 // metres across
	compassWidth = 420
	compassSpan  = 180 // degrees across the compass
)

// mark is something on the map: a footprint, X across and Y for the
// game's Z, in metres, and its colour.
type mark struct {
	r rl.Rectangle
	c rl.Color
}

// label names a place on the maps, at a point in the game's frame (X, Z).
// The more important a place (the lower its tier), the bigger its name and
// the sooner it's drawn: where names would overlap, the later one is left
// out, so zoomed out the seats are named, and zoomed in what's in them.
type label struct {
	name string
	at   rl.Vector2
	tier tier
}

type tier uint8

const (
	seat  tier = iota // a seat: marked, and named in big letters
	route             // a road or way: named along it, unmarked
	spot              // a place in a seat: marked, and named small
)

// worldMap is a resource: what the maps draw, and where the full map is
// looking.
type worldMap struct {
	marks  []mark
	bounds rl.Rectangle
	// The full map's middle (in metres) and scale (pixels per metre, at
	// the UI's scale of 1); zero until it's first opened.
	at   rl.Vector2
	zoom float32
}

// The map's colours: the Fringe's ground, the ground inside the dome, the
// floors, and each part of the world.
var (
	mapFringe  = rl.NewColor(74, 42, 32, 255)
	mapInside  = rl.NewColor(104, 62, 46, 255)
	mapDeck    = rl.NewColor(64, 64, 70, 255)
	mapPads    = rl.NewColor(92, 92, 96, 255)
	mapRoad    = rl.NewColor(148, 118, 98, 255)
	mapDefault = rl.NewColor(170, 160, 150, 255)
	mapColours = map[string]rl.Color{
		"The Hull":        rl.NewColor(150, 150, 158, 255),
		"The Hull market": rl.NewColor(222, 142, 60, 255),
		"Hull decks":      rl.NewColor(196, 118, 66, 255),
		"The Exchange":    rl.NewColor(214, 180, 96, 255),
		"Charter Row":     rl.NewColor(236, 236, 240, 255),
		"The Pads":        rl.NewColor(122, 146, 180, 255),
		"Domes and gate":  rl.NewColor(160, 206, 224, 255),
		"The Fringe":      rl.NewColor(178, 124, 92, 255),
		"Vehicles":        rl.NewColor(236, 196, 92, 255),
	}
)

// The dome line, as tools/world/landfall.py has it (DOME_*), in the game's
// frame.
var domeArea = rl.Rectangle{X: -54, Y: -56, Width: 88, Height: 100}

// places are the maps' names, at landfall.py's districts in the game's
// frame (Blender's (x, y) is the game's (x, -y); each seat's own layout is
// moved to its middle).
var places = []label{
	{"Landfall", rl.Vector2{}, seat},
	{"The Pads", padsAt, seat},
	{"The Fringers' hold", holdAt, seat},
	{"The Quiet Book's haven", havenAt, seat},
	{"Caravan road", rl.Vector2{X: 5600, Y: -540}, route},
	{"Fringe track", rl.Vector2{X: -1260, Y: 7600}, route},
	{"The wash", rl.Vector2{X: -5900, Y: -6100}, route},
	// In Landfall.
	{"The Hull", rl.Vector2{X: -16, Y: -16}, spot}, // just north of it
	{"The Exchange", rl.Vector2{X: 2, Y: 4}, spot},
	{"Lower decks", rl.Vector2{X: -34, Y: 4}, spot},
	{"The Stacks", rl.Vector2{X: -15, Y: -6}, spot},
	{"Hull market", rl.Vector2{X: -16, Y: 14}, spot},
	{"Charter Row", rl.Vector2{X: -4, Y: -36}, spot},
	{"South gate", rl.Vector2{X: 0, Y: 46}, spot},
	{"Caravan stop", rl.Vector2{X: -6, Y: 54}, spot},
	// At the Pads.
	{"The drifter", rl.Vector2{X: 9807, Y: -1798}, spot},
	{"Drifter market", rl.Vector2{X: 9807, Y: -1768}, spot},
	{"Container yard", rl.Vector2{X: 9784, Y: -1805}, spot},
	{"Arrivals", rl.Vector2{X: 9770, Y: -1770}, spot},
	{"Fuel depot", rl.Vector2{X: 9810, Y: -1839}, spot},
	{"Vehicle hire", rl.Vector2{X: 9768, Y: -1788}, spot},
	// Along the roads.
	{"Fuel stop", rl.Vector2{X: 5600, Y: -518}, spot},
	{"Broken-down hauler", rl.Vector2{X: 7600, Y: -1262}, spot},
	{"Scrap cairn", rl.Vector2{X: -1291, Y: 7600}, spot},
	// In the hold.
	{"Farms", rl.Vector2{X: -1625, Y: 12023}, spot},
	{"Salvage fields", rl.Vector2{X: -1375, Y: 12020}, spot},
	{"Fringer camp", rl.Vector2{X: -1460, Y: 12014}, spot},
	{"Wind farm", rl.Vector2{X: -1600, Y: 11915}, spot},
	{"Terraformer wreck", rl.Vector2{X: -1430, Y: 12042}, spot},
}

// mapLabels is how big each tier's names are on a map, in points.
type mapLabels [3]float32

// drawLabels names the places in the frame, the most important first,
// leaving out any name that would cover one already drawn.
func (f mapFrame) drawLabels(p painter, sizes mapLabels) {
	var taken []rl.Rectangle
	mark := p.px(sizes[spot] * .4)
	for t := seat; t <= spot; t++ {
		for _, l := range places {
			if l.tier != t {
				continue
			}
			pos := f.toScreen(l.at)
			if pos.X < f.screen.X || pos.Y < f.screen.Y || pos.X > f.screen.X+f.screen.Width || pos.Y > f.screen.Y+f.screen.Height {
				continue
			}
			size := sizes[t]
			m := p.measure(l.name, size, semibold)
			// Marked places are named just above their mark; a route over
			// its middle.
			r := rl.Rectangle{X: pos.X - m.X/2, Y: pos.Y - m.Y/2, Width: m.X, Height: m.Y}
			if t != route {
				r.Y = pos.Y - mark - m.Y
			}
			// Kept within the frame.
			r.X = min(f.screen.X+f.screen.Width-r.Width, max(f.screen.X, r.X))
			r.Y = min(f.screen.Y+f.screen.Height-r.Height, max(f.screen.Y, r.Y))
			if slices.ContainsFunc(taken, func(o rl.Rectangle) bool { return rl.CheckCollisionRecs(o, r) }) {
				continue
			}
			taken = append(taken, r)
			if t != route {
				m := mark
				if t == seat {
					m *= 1.4
				}
				rl.DrawRectangleRec(rl.Rectangle{X: pos.X - m/2 - 1, Y: pos.Y - m/2 - 1, Width: m + 2, Height: m + 2}, rl.NewColor(0, 0, 0, 200))
				c := colText
				if t == seat {
					c = colAccent
				}
				rl.DrawRectangleRec(rl.Rectangle{X: pos.X - m/2, Y: pos.Y - m/2, Width: m, Height: m}, c)
			}
			c := colText
			if t == route {
				c = colMuted
			}
			shadow := max(1, p.px(1))
			p.text(l.name, rl.Vector2{X: r.X + shadow, Y: r.Y + shadow}, size, semibold, rl.NewColor(0, 0, 0, 200))
			p.text(l.name, rl.Vector2{X: r.X, Y: r.Y}, size, semibold, c)
		}
	}
}

// newWorldMap is the map of the pieces in the layout at path.
func newWorldMap(k *world.Kit, placed []world.Placement) *worldMap {
	h := float32(worldSize) / 2
	m := &worldMap{bounds: rl.Rectangle{X: -h, Y: -h, Width: 2 * h, Height: 2 * h}}
	m.marks = append(m.marks, mark{domeArea, mapInside})
	m.marks = append(m.marks, mark{floored[0], mapDeck}, mark{floored[1], mapPads})
	type tall struct {
		mark
		top float32
	}
	var pieces []tall
	for _, p := range placed {
		piece, ok := k.Pieces[p.Piece]
		if !ok {
			continue
		}
		c, ok := mapColours[piece.Category]
		if !ok {
			c = mapDefault
		}
		at := rl.Vector3{X: p.At[0], Y: p.At[1], Z: p.At[2]}
		turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
		for _, col := range piece.Colliders {
			r, top := footprint(col, at, turn)
			// Taller things are lighter, so walls stand out from what's
			// on the floor.
			pieces = append(pieces, tall{mark{r, lighten(c, min(top/8, 1)*.35)}, top})
		}
	}
	// Low things first, so taller ones are drawn over them.
	slices.SortStableFunc(pieces, func(a, b tall) int { return cmp.Compare(a.top, b.top) })
	for _, p := range pieces {
		m.marks = append(m.marks, p.mark)
	}
	return m
}

// footprint is the rectangle on the ground a collider of a piece at at,
// turned by turn, covers, and how high it reaches.
func footprint(c world.Collider, at rl.Vector3, turn rl.Quaternion) (rl.Rectangle, float32) {
	center, rot := c.In(at, turn)
	lo := rl.Vector2{X: float32(math.Inf(1)), Y: float32(math.Inf(1))}
	hi := rl.Vector2{X: float32(math.Inf(-1)), Y: float32(math.Inf(-1))}
	top := float32(math.Inf(-1))
	for _, sx := range []float32{-1, 1} {
		for _, sy := range []float32{-1, 1} {
			for _, sz := range []float32{-1, 1} {
				corner := rl.Vector3{X: sx * c.Size[0] / 2, Y: sy * c.Size[1] / 2, Z: sz * c.Size[2] / 2}
				p := rl.Vector3Add(center, rl.Vector3RotateByQuaternion(corner, rot))
				lo = rl.Vector2{X: min(lo.X, p.X), Y: min(lo.Y, p.Z)}
				hi = rl.Vector2{X: max(hi.X, p.X), Y: max(hi.Y, p.Z)}
				top = max(top, p.Y)
			}
		}
	}
	return rl.Rectangle{X: lo.X, Y: lo.Y, Width: hi.X - lo.X, Height: hi.Y - lo.Y}, top
}

func lighten(c rl.Color, by float32) rl.Color {
	mix := func(v uint8) uint8 { return uint8(float32(v) + (255-float32(v))*by) }
	return rl.NewColor(mix(c.R), mix(c.G), mix(c.B), c.A)
}

// mapFrame is where the map is drawn: the part of the screen, the point
// of the world in its middle and how many pixels a metre is.
type mapFrame struct {
	screen rl.Rectangle
	at     rl.Vector2
	scale  float32
}

// toScreen is where a point of the world (X, Z) is in the frame.
func (f mapFrame) toScreen(p rl.Vector2) rl.Vector2 {
	return rl.Vector2{
		X: f.screen.X + f.screen.Width/2 + (p.X-f.at.X)*f.scale,
		Y: f.screen.Y + f.screen.Height/2 + (p.Y-f.at.Y)*f.scale,
	}
}

// draw draws the map's marks in the frame, cut to its edges.
func (f mapFrame) draw(m *worldMap) {
	rl.DrawRectangleRec(f.screen, mapFringe)
	for _, mk := range m.marks {
		a := f.toScreen(rl.Vector2{X: mk.r.X, Y: mk.r.Y})
		r := clip(rl.Rectangle{X: a.X, Y: a.Y, Width: mk.r.Width * f.scale, Height: mk.r.Height * f.scale}, f.screen)
		// Anything thinner than a pixel still shows, as one.
		if r.Width > 0 && r.Height > 0 {
			r.Width, r.Height = max(r.Width, 1), max(r.Height, 1)
			rl.DrawRectangleRec(r, mk.c)
		}
	}
}

// drawRoads draws the roads as lines, cut to the frame's edges.
func (f mapFrame) drawRoads(width float32) {
	for _, r := range roads {
		if !r.paint {
			continue
		}
		w := max(width, 2*r.half*f.scale)
		for i := 1; i < len(r.points); i++ {
			a, b := f.toScreen(r.points[i-1]), f.toScreen(r.points[i])
			if a, b, ok := clipSegment(a, b, f.screen); ok {
				stroke(a, b, w, mapRoad)
			}
		}
	}
}

// clipSegment is the part of a to b inside r, if any (Liang-Barsky).
func clipSegment(a, b rl.Vector2, r rl.Rectangle) (rl.Vector2, rl.Vector2, bool) {
	t0, t1 := float32(0), float32(1)
	d := rl.Vector2Subtract(b, a)
	for _, e := range [4][2]float32{{-d.X, a.X - r.X}, {d.X, r.X + r.Width - a.X}, {-d.Y, a.Y - r.Y}, {d.Y, r.Y + r.Height - a.Y}} {
		p, q := e[0], e[1]
		if p == 0 {
			if q < 0 {
				return a, b, false
			}
			continue
		}
		t := q / p
		if p < 0 {
			t0 = max(t0, t)
		} else {
			t1 = min(t1, t)
		}
		if t0 > t1 {
			return a, b, false
		}
	}
	return rl.Vector2Add(a, rl.Vector2Scale(d, t0)), rl.Vector2Add(a, rl.Vector2Scale(d, t1)), true
}

// clip is r cut to within to: empty if they don't meet.
func clip(r, to rl.Rectangle) rl.Rectangle {
	x0, y0 := max(r.X, to.X), max(r.Y, to.Y)
	x1, y1 := min(r.X+r.Width, to.X+to.Width), min(r.Y+r.Height, to.Y+to.Height)
	if x1 <= x0 || y1 <= y0 {
		return rl.Rectangle{}
	}
	return rl.Rectangle{X: x0, Y: y0, Width: x1 - x0, Height: y1 - y0}
}

// you draws the player as an arrow at at, pointing along the yaw facing
// (radians about Up, 0 facing +Z: down the map).
func you(at rl.Vector2, facing, size float32) {
	dir := rl.Vector2{X: float32(math.Sin(float64(facing))), Y: float32(math.Cos(float64(facing)))}
	side := rl.Vector2{X: -dir.Y, Y: dir.X}
	tip := rl.Vector2Add(at, rl.Vector2Scale(dir, size))
	back := rl.Vector2Subtract(at, rl.Vector2Scale(dir, size*.6))
	l := rl.Vector2Add(back, rl.Vector2Scale(side, size*.7))
	r := rl.Vector2Subtract(back, rl.Vector2Scale(side, size*.7))
	rl.DrawCircle(int32(at.X), int32(at.Y), size*.95, rl.NewColor(0, 0, 0, 120))
	for _, s := range [][2]rl.Vector2{{l, tip}, {r, tip}, {l, at}, {r, at}} {
		stroke(s[0], s[1], max(2, size*.32), colAccent)
	}
}

// player is where the player is on the map (X, Z) and the way their body
// faces, as a yaw.
func player(players *illusion.Query2Where[transform.Transform, character.MotionSamples, illusion.With[character.Player]], bodies *illusion.Query1Where[transform.Transform, illusion.With[character.Body]], hier *illusion.Hierarchy) (rl.Vector2, float32, bool) {
	root, tr, _, ok := players.Single()
	if !ok {
		return rl.Vector2{}, 0, false
	}
	var facing float32
	bodies.Each(func(e ecs.Entity, b *transform.Transform) {
		if parent, ok := hier.Parent(e); ok && parent == root {
			f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, b.Rotation)
			facing = float32(math.Atan2(float64(f.X), float64(f.Z)))
		}
	})
	return rl.Vector2{X: tr.Translation.X, Y: tr.Translation.Z}, facing, true
}

// heading is the way the camera looks, as a compass bearing in degrees:
// 0 north (the game's -Z), 90 east (+X).
func heading(forward rl.Vector3) float32 {
	b := float32(math.Atan2(float64(forward.X), float64(-forward.Z))) * 180 / math.Pi
	if b < 0 {
		b += 360
	}
	return b
}

// mapPlayers is the player and their body, for the maps (one parameter,
// to stay within Fn8).
type mapPlayers struct {
	roots  illusion.Query2Where[transform.Transform, character.MotionSamples, illusion.With[character.Player]]
	bodies illusion.Query1Where[transform.Transform, illusion.With[character.Body]]
	hier   illusion.Hierarchy
}

func (p *mapPlayers) InitParam(w *ecs.World) {
	p.roots.InitParam(w)
	p.bodies.InitParam(w)
	p.hier.InitParam(w)
}

// drawMaps draws, in play, the minimap and the compass, and on the map
// screen, the full map.
func drawMaps(
	win *illusion.Res[window.Window],
	fonts *illusion.Res[uiFonts],
	m *illusion.Res[menu],
	wm *illusion.Res[worldMap],
	view *illusion.Res[character.View],
	ps *mapPlayers,
) {
	wmap, ok := wm.TryGet()
	if !ok {
		return
	}
	at, facing, ok := player(&ps.roots, &ps.bodies, &ps.hier)
	if !ok {
		return
	}
	ww := win.Get()
	p := newPainter(fonts.Get(), ww)
	width, height := float32(ww.Width), float32(ww.Height)
	switch m.Get().screen() {
	case playing:
		drawMinimap(p, wmap, at, facing)
		drawCompass(p, heading(view.Get().Forward), width)
	case mapping:
		drawFullMap(p, wmap, at, facing, width, height)
	}
}

// minimapRect is where the minimap goes: the top left corner.
func minimapRect(p painter) rl.Rectangle {
	s := p.px(minimapSize)
	return rl.Rectangle{X: p.px(16), Y: p.px(16), Width: s, Height: s}
}

func drawMinimap(p painter, m *worldMap, at rl.Vector2, facing float32) {
	r := minimapRect(p)
	border := max(1, p.px(3))
	rl.DrawRectangleRec(inset(r, -border, -border), colPanel)
	f := mapFrame{screen: r, at: at, scale: r.Width / minimapRange}
	f.draw(m)
	f.drawRoads(max(2, p.px(2)))
	f.drawLabels(p, mapLabels{12, 10, 10})
	you(f.toScreen(at), facing, p.px(5))
	// North, and the key for the full map, under it.
	p.textIn("N", rl.Rectangle{X: r.X + r.Width - p.px(16), Y: r.Y + p.px(2), Width: p.px(14), Height: p.px(14)}, 12, semibold, colAccent, centre)
	x, y := r.X, r.Y+r.Height+border+p.px(6)
	x += p.keycap("M", rl.Vector2{X: x, Y: y}, 11) + p.px(6)
	p.text("Map", rl.Vector2{X: x, Y: y + p.px(2)}, 12, semibold, colText)
}

// drawCompass draws the strip along the top: the bearings round the way
// the camera looks, with the cardinal points named.
func drawCompass(p painter, bearing, width float32) {
	w, h := p.px(compassWidth), p.px(34)
	r := rl.Rectangle{X: (width - w) / 2, Y: p.px(14), Width: w, Height: h}
	rl.DrawRectangleRec(r, colPanel)
	perDeg := w / compassSpan
	names := map[int]string{0: "N", 45: "NE", 90: "E", 135: "SE", 180: "S", 225: "SW", 270: "W", 315: "NW"}
	for d := 0; d < 360; d += 15 {
		off := float32(d) - bearing
		off -= 360 * float32(math.Round(float64(off/360)))
		if math.Abs(float64(off)) > compassSpan/2-4 {
			continue
		}
		x := r.X + w/2 + off*perDeg
		name, cardinal := names[d]
		if cardinal {
			size, c := float32(13), colMuted
			if d%90 == 0 {
				size, c = 16, colText
			}
			if d == 0 {
				c = colAccent
			}
			p.textIn(name, rl.Rectangle{X: x - p.px(20), Y: r.Y, Width: p.px(40), Height: h}, size, semibold, c, centre)
			continue
		}
		rl.DrawRectangleRec(rl.Rectangle{X: x - max(.5, p.px(.75)), Y: r.Y + h*.35, Width: max(1, p.px(1.5)), Height: h * .3}, colMuted)
	}
	// Where it's pointing, and the bearing under it.
	rl.DrawRectangleRec(rl.Rectangle{X: r.X + w/2 - max(1, p.px(1)), Y: r.Y + h - p.px(5), Width: max(2, p.px(2)), Height: p.px(9)}, colAccent)
	p.textIn(fmt.Sprintf("%03.0f", bearing), rl.Rectangle{X: r.X + w/2 - p.px(30), Y: r.Y + h + p.px(4), Width: p.px(60), Height: p.px(18)}, 13, semibold, colMuted, centre)
}

// fullMapRect is where the full map goes: most of the screen.
func fullMapRect(p painter, width, height float32) rl.Rectangle {
	return inset(rl.Rectangle{Width: width, Height: height}, p.px(48), p.px(48))
}

func drawFullMap(p painter, m *worldMap, at rl.Vector2, facing, width, height float32) {
	rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 160))
	r := fullMapRect(p, width, height)
	rl.DrawRectangleRec(inset(r, -p.px(8), -p.px(8)), colPanel)
	f := mapFrame{screen: r, at: m.at, scale: m.zoom * p.s}
	f.draw(m)
	f.drawRoads(max(2, p.px(2)))
	f.drawLabels(p, mapLabels{20, 14, 14})
	if pos := f.toScreen(at); clip(rl.Rectangle{X: pos.X, Y: pos.Y, Width: 1, Height: 1}, r).Width > 0 {
		you(pos, facing, p.px(7))
	}
	p.text("The Fringe", rl.Vector2{X: r.X + p.px(16), Y: r.Y + p.px(12)}, 30, black, colText)
	x, y := r.X+p.px(16), r.Y+r.Height-p.px(14*1.7)-p.px(14)
	for _, k := range []struct{ key, does string }{{"M", "Close"}, {"Esc", "Close"}, {"Drag", "Move"}, {"Scroll", "Zoom"}} {
		x += p.keycap(k.key, rl.Vector2{X: x, Y: y}, 14) + p.px(8)
		p.text(k.does, rl.Vector2{X: x, Y: y + p.px(4)}, 15, semibold, colText)
		x += p.measure(k.does, 15, semibold).X + p.px(20)
	}
}

// Full map zoom, in pixels per metre at the UI's scale of 1.
const (
	mapZoomMin = .03 // the whole region
	mapZoomMax = 16
	mapZoomOut = .1 // where it opens: 10 km or so across
)

// mapInput moves the full map: drag to move it, scroll to zoom about the
// pointer. It opens on the player.
func mapInput(
	m *illusion.Res[menu],
	wm *illusion.Res[worldMap],
	mouse *illusion.Res[input.Mouse],
	buttons *illusion.Res[input.MouseButtons],
	win *illusion.Res[window.Window],
	fonts *illusion.Res[uiFonts],
	ps *mapPlayers,
	opened *illusion.Local[bool],
) {
	wmap, ok := wm.TryGet()
	if !ok {
		return
	}
	if m.Get().screen() != mapping {
		*opened.Get() = false
		return
	}
	if !*opened.Get() {
		*opened.Get() = true
		if at, _, ok := player(&ps.roots, &ps.bodies, &ps.hier); ok {
			wmap.at = at
		}
		wmap.zoom = mapZoomOut
	}
	p := newPainter(fonts.Get(), win.Get())
	ms := mouse.Get()
	scale := wmap.zoom * p.s
	if buttons.Get().AnyPressed(rl.MouseButtonLeft, rl.MouseButtonRight) {
		wmap.at = rl.Vector2Subtract(wmap.at, rl.Vector2Scale(ms.Delta, 1/scale))
	}
	if ms.Wheel != 0 {
		ww := win.Get()
		r := fullMapRect(p, float32(ww.Width), float32(ww.Height))
		middle := rl.Vector2{X: r.X + r.Width/2, Y: r.Y + r.Height/2}
		// Keep the point under the pointer where it is.
		under := rl.Vector2Add(wmap.at, rl.Vector2Scale(rl.Vector2Subtract(ms.Position, middle), 1/scale))
		wmap.zoom = min(mapZoomMax, max(mapZoomMin, wmap.zoom*float32(math.Pow(1.15, float64(ms.Wheel)))))
		wmap.at = rl.Vector2Subtract(under, rl.Vector2Scale(rl.Vector2Subtract(ms.Position, middle), 1/(wmap.zoom*p.s)))
	}
	// Not off the world.
	b := wmap.bounds
	wmap.at = rl.Vector2{X: min(b.X+b.Width, max(b.X, wmap.at.X)), Y: min(b.Y+b.Height, max(b.Y, wmap.at.Y))}
}
