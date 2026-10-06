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
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// The maps: a minimap in the top left corner in play, a compass along the top, and
// the full map, which M opens and closes (as a screen: it pauses, like the
// menus). Both maps are drawn from the layout, each piece as the footprint
// of its colliders, coloured by the part of the world it's from. The
// minimap turns with the camera, so what's ahead is up; the full map has
// north (the game's -Z) up.
//
// A click on the full map marks a destination (another click on it takes
// it off). It's shown on both maps, on the compass and over the world, with
// how far it is, until the player gets there.

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
	// dest is the destination marked on the map (X, Z), if marked.
	dest   rl.Vector2
	marked bool
}

// arrived is how near the destination, in metres, the player has to get
// for it to be reached, and taken off the map.
const arrived = 6

// markReach is how near a click has to be to the destination's mark, in
// points, to take it off rather than move it.
const markReach = 14

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
// of the world in its middle, how many pixels a metre is, and the way in
// the world (X, Z) that's up on it: north, unless it turns.
type mapFrame struct {
	screen rl.Rectangle
	at     rl.Vector2
	scale  float32
	up     rl.Vector2 // a unit vector; zero for north
}

// north is the way north is in the world (X, Z): the game's -Z.
var north = rl.Vector2{Y: -1}

// axes are the ways in the world that are right and up on the frame.
func (f mapFrame) axes() (right, up rl.Vector2) {
	up = f.up
	if up == (rl.Vector2{}) {
		up = north
	}
	return rl.Vector2{X: -up.Y, Y: up.X}, up
}

// turned reports whether the frame turns away from north up.
func (f mapFrame) turned() bool { return f.up != (rl.Vector2{}) && f.up != north }

// middle is the frame's middle on the screen.
func (f mapFrame) middle() rl.Vector2 {
	return rl.Vector2{X: f.screen.X + f.screen.Width/2, Y: f.screen.Y + f.screen.Height/2}
}

// toScreen is where a point of the world (X, Z) is in the frame.
func (f mapFrame) toScreen(p rl.Vector2) rl.Vector2 {
	return rl.Vector2Add(f.middle(), rl.Vector2Scale(f.screenDir(rl.Vector2Subtract(p, f.at)), f.scale))
}

// toWorld is the point of the world (X, Z) at a point of the frame.
func (f mapFrame) toWorld(s rl.Vector2) rl.Vector2 {
	right, up := f.axes()
	d := rl.Vector2Scale(rl.Vector2Subtract(s, f.middle()), 1/f.scale)
	return rl.Vector2Add(f.at, rl.Vector2Add(rl.Vector2Scale(right, d.X), rl.Vector2Scale(up, -d.Y)))
}

// screenDir is a way in the world (X, Z) as a way on the frame.
func (f mapFrame) screenDir(v rl.Vector2) rl.Vector2 {
	right, up := f.axes()
	return rl.Vector2{X: rl.Vector2DotProduct(v, right), Y: -rl.Vector2DotProduct(v, up)}
}

// draw draws the map's marks in the frame, cut to its edges. (A turned
// frame's marks are turned rectangles, drawn whole: draw it in scissor
// mode, cut to the frame.)
func (f mapFrame) draw(m *worldMap) {
	rl.DrawRectangleRec(f.screen, mapFringe)
	if f.turned() {
		f.drawTurned(m)
		return
	}
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

// drawTurned draws the marks turned with the frame: those that reach
// within the circle round it.
func (f mapFrame) drawTurned(m *worldMap) {
	x := f.screenDir(rl.Vector2{X: 1})
	angle := float32(math.Atan2(float64(x.Y), float64(x.X))) * 180 / math.Pi
	reach := float32(math.Hypot(float64(f.screen.Width), float64(f.screen.Height))) / 2 / f.scale
	for _, mk := range m.marks {
		half := rl.Vector2{X: mk.r.Width / 2, Y: mk.r.Height / 2}
		centre := rl.Vector2{X: mk.r.X + half.X, Y: mk.r.Y + half.Y}
		if rl.Vector2Distance(centre, f.at)-rl.Vector2Length(half) > reach {
			continue
		}
		w, h := max(mk.r.Width*f.scale, 1), max(mk.r.Height*f.scale, 1)
		pos := f.toScreen(centre)
		rl.DrawRectanglePro(rl.Rectangle{X: pos.X, Y: pos.Y, Width: w, Height: h}, rl.Vector2{X: w / 2, Y: h / 2}, angle, mk.c)
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

// you draws the player as an arrow at at, pointing along dir (on the
// screen, a unit vector).
func you(at, dir rl.Vector2, size float32) {
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

// player is where the player is on the map (X, Z), how high, and the way
// their body faces (X, Z), a unit vector.
func player(players *illusion.Query2Where[transform.Transform, character.MotionSamples, illusion.With[character.Player]], bodies *illusion.Query1Where[transform.Transform, illusion.With[character.Body]], hier *illusion.Hierarchy) (at rl.Vector2, y float32, facing rl.Vector2, ok bool) {
	root, tr, _, ok := players.Single()
	if !ok {
		return rl.Vector2{}, 0, rl.Vector2{}, false
	}
	facing = rl.Vector2{Y: 1}
	bodies.Each(func(e ecs.Entity, b *transform.Transform) {
		if parent, ok := hier.Parent(e); ok && parent == root {
			facing = facingOf(tr.Rotation, b.Rotation)
		}
	})
	return rl.Vector2{X: tr.Translation.X, Y: tr.Translation.Z}, tr.Translation.Y, facing, true
}

// facingOf is the way (X, Z) a body turned by body, in a root turned by
// root, faces: on foot the body turns; seated, the root turns with what
// carries it.
func facingOf(root, body rl.Quaternion) rl.Vector2 {
	f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, rl.QuaternionMultiply(root, body))
	if d := (rl.Vector2{X: f.X, Y: f.Z}); rl.Vector2Length(d) > 1e-4 {
		return rl.Vector2Normalize(d)
	}
	return rl.Vector2{Y: 1}
}

// looking is the way the camera looks as it's drawn (it eases after where
// it's steered, and follows a vehicle round), or else where it's steered.
func looking(v *render.View3D, steered rl.Vector3) rl.Vector3 {
	if v.Active {
		if f := rl.Vector3Subtract(v.Camera.Target, v.Camera.Position); rl.Vector2Length(rl.Vector2{X: f.X, Y: f.Z}) > 1e-4 {
			return rl.Vector3Normalize(f)
		}
	}
	return steered
}

// edgePoint is where a line from the middle of r along dir meets r's edge,
// pad in from it.
func edgePoint(r rl.Rectangle, dir rl.Vector2, pad float32) rl.Vector2 {
	mid := rl.Vector2{X: r.X + r.Width/2, Y: r.Y + r.Height/2}
	hw, hh := r.Width/2-pad, r.Height/2-pad
	t := float32(math.Inf(1))
	if dir.X != 0 {
		t = hw / float32(math.Abs(float64(dir.X)))
	}
	if dir.Y != 0 {
		t = min(t, hh/float32(math.Abs(float64(dir.Y))))
	}
	if math.IsInf(float64(t), 1) {
		return mid
	}
	return rl.Vector2Add(mid, rl.Vector2Scale(dir, t))
}

// within reports whether p is in r, pad in from its edges.
func within(r rl.Rectangle, p rl.Vector2, pad float32) bool {
	return p.X >= r.X+pad && p.X <= r.X+r.Width-pad && p.Y >= r.Y+pad && p.Y <= r.Y+r.Height-pad
}

// distance is how far a way is, for the destination: to the 10 m under a
// kilometre, and in tenths of a kilometre over.
func distance(m float32) string {
	switch {
	case m < 100:
		return fmt.Sprintf("%.0f m", m)
	case m < 995:
		return fmt.Sprintf("%.0f m", math.Round(float64(m)/10)*10)
	default:
		return fmt.Sprintf("%.1f km", m/1000)
	}
}

// destMark draws the destination's mark: a diamond, size across, centred
// at at.
func destMark(at rl.Vector2, size float32) {
	o := size * 1.35
	rl.DrawRectanglePro(rl.Rectangle{X: at.X, Y: at.Y, Width: o, Height: o}, rl.Vector2{X: o / 2, Y: o / 2}, 45, rl.NewColor(0, 0, 0, 220))
	rl.DrawRectanglePro(rl.Rectangle{X: at.X, Y: at.Y, Width: size, Height: size}, rl.Vector2{X: size / 2, Y: size / 2}, 45, colDest)
	d := size * .3
	rl.DrawRectanglePro(rl.Rectangle{X: at.X, Y: at.Y, Width: d, Height: d}, rl.Vector2{X: d / 2, Y: d / 2}, 45, rl.NewColor(0, 0, 0, 220))
}

// colDest is the destination's colour: bright against the map's reds and
// the accent's orange.
var colDest = rl.NewColor(120, 220, 255, 255)

// drawRoute draws the way from the player to the destination in the frame:
// a line, and the mark at its end, or at the frame's edge if it's off it.
func (f mapFrame) drawRoute(m *worldMap, from rl.Vector2, line, size float32) {
	if !m.marked {
		return
	}
	a, b := f.toScreen(from), f.toScreen(m.dest)
	if a, b, ok := clipSegment(a, b, f.screen); ok {
		stroke(a, b, line, rl.NewColor(colDest.R, colDest.G, colDest.B, 150))
	}
	if !within(f.screen, b, size) {
		b = edgePoint(f.screen, rl.Vector2Normalize(rl.Vector2Subtract(b, f.middle())), size)
	}
	destMark(b, size)
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

// mapPlayers is the player and their body, and the camera's view, for the
// maps (one parameter, to stay within Fn8).
type mapPlayers struct {
	roots  illusion.Query2Where[transform.Transform, character.MotionSamples, illusion.With[character.Player]]
	bodies illusion.Query1Where[transform.Transform, illusion.With[character.Body]]
	hier   illusion.Hierarchy
	view   illusion.Res[render.View3D]
}

func (p *mapPlayers) InitParam(w *ecs.World) {
	p.roots.InitParam(w)
	p.bodies.InitParam(w)
	p.hier.InitParam(w)
	p.view.InitParam(w)
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
	at, y, facing, ok := player(&ps.roots, &ps.bodies, &ps.hier)
	if !ok {
		return
	}
	ww := win.Get()
	p := newPainter(fonts.Get(), ww)
	width, height := float32(ww.Width), float32(ww.Height)
	switch m.Get().screen() {
	case playing:
		forward := looking(ps.view.Get(), view.Get().Forward)
		if wmap.marked {
			drawDestination(p, ps.view.Get(), wmap.dest, at, y, width, height)
		}
		drawMinimap(p, wmap, at, facing, rl.Vector2{X: forward.X, Y: forward.Z})
		var dest *rl.Vector2
		if wmap.marked {
			d := rl.Vector2Subtract(wmap.dest, at)
			dest = &d
		}
		drawCompass(p, heading(forward), width, dest)
	case mapping:
		drawFullMap(p, wmap, at, facing, width, height)
	}
}

// drawDestination marks the destination over the world, with how far it
// is: where it is on the screen, or at the screen's edge, the way to turn,
// if it's out of view.
func drawDestination(p painter, v *render.View3D, dest, at rl.Vector2, y, width, height float32) {
	if !v.Active {
		return
	}
	cam := v.Camera
	point := rl.Vector3{X: dest.X, Y: max(groundHeight(dest.X, dest.Y), y-1) + 2, Z: dest.Y}
	fwd := rl.Vector3Normalize(rl.Vector3Subtract(cam.Target, cam.Position))
	right := rl.Vector3Normalize(rl.Vector3CrossProduct(fwd, cam.Up))
	up := rl.Vector3CrossProduct(right, fwd)
	d := rl.Vector3Subtract(point, cam.Position)
	screen := rl.Rectangle{Width: width, Height: height}
	pad := p.px(40)
	pos := v.WorldToScreen(point)
	if rl.Vector3DotProduct(d, fwd) <= .5 || !within(screen, pos, pad) {
		dir := rl.Vector2{X: rl.Vector3DotProduct(d, right), Y: -rl.Vector3DotProduct(d, up)}
		if rl.Vector3DotProduct(d, fwd) <= .5 {
			// Behind: to the side it's on, along the bottom.
			dir.Y = max(dir.Y, float32(math.Abs(float64(dir.X)))*.5+1)
		}
		if rl.Vector2Length(dir) < 1e-4 {
			dir = rl.Vector2{Y: 1}
		}
		pos = edgePoint(screen, rl.Vector2Normalize(dir), pad)
	}
	destMark(pos, p.px(14))
	text := distance(rl.Vector2Distance(at, dest))
	m := p.measure(text, 14, semibold)
	tp := rl.Vector2{X: pos.X - m.X/2, Y: pos.Y + p.px(14)}
	shadow := max(1, p.px(1))
	p.text(text, rl.Vector2{X: tp.X + shadow, Y: tp.Y + shadow}, 14, semibold, rl.NewColor(0, 0, 0, 200))
	p.text(text, tp, 14, semibold, colText)
}

// minimapRect is where the minimap goes: the top left corner.
func minimapRect(p painter) rl.Rectangle {
	s := p.px(minimapSize)
	return rl.Rectangle{X: p.px(16), Y: p.px(16), Width: s, Height: s}
}

// drawMinimap draws the minimap round the player, turned so the way the
// camera looks (X, Z) is up.
func drawMinimap(p painter, m *worldMap, at, facing, looking rl.Vector2) {
	r := minimapRect(p)
	border := max(1, p.px(3))
	rl.DrawRectangleRec(inset(r, -border, -border), colPanel)
	f := mapFrame{screen: r, at: at, scale: r.Width / minimapRange}
	if rl.Vector2Length(looking) > 1e-4 {
		f.up = rl.Vector2Normalize(looking)
	}
	rl.BeginScissorMode(int32(r.X), int32(r.Y), int32(math.Ceil(float64(r.Width))), int32(math.Ceil(float64(r.Height))))
	f.draw(m)
	f.drawRoads(max(2, p.px(2)))
	f.drawLabels(p, mapLabels{12, 10, 10})
	rl.EndScissorMode()
	f.drawRoute(m, at, max(1.5, p.px(2)), p.px(10))
	you(f.toScreen(at), f.screenDir(facing), p.px(5))
	// North, at the edge the way it is, and the key for the full map
	// under it.
	n := edgePoint(r, rl.Vector2Normalize(f.screenDir(north)), p.px(9))
	rl.DrawRectangleRec(rl.Rectangle{X: n.X - p.px(8), Y: n.Y - p.px(8), Width: p.px(16), Height: p.px(16)}, rl.NewColor(0, 0, 0, 170))
	p.textIn("N", rl.Rectangle{X: n.X - p.px(8), Y: n.Y - p.px(8), Width: p.px(16), Height: p.px(16)}, 12, semibold, colAccent, centre)
	x, y := r.X, r.Y+r.Height+border+p.px(6)
	x += p.keycap("M", rl.Vector2{X: x, Y: y}, 11) + p.px(6)
	p.text("Map", rl.Vector2{X: x, Y: y + p.px(2)}, 12, semibold, colText)
}

// drawCompass draws the strip along the top: the bearings round the way
// the camera looks, with the cardinal points named, and the way to the
// destination (X, Z from the player), if there is one.
func drawCompass(p painter, bearing, width float32, dest *rl.Vector2) {
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
	// The way to the destination, at the edge if it's off the strip.
	if dest != nil {
		off := heading(rl.Vector3{X: dest.X, Z: dest.Y}) - bearing
		off -= 360 * float32(math.Round(float64(off/360)))
		limit := float32(compassSpan/2 - 6)
		off = max(-limit, min(limit, off))
		destMark(rl.Vector2{X: r.X + w/2 + off*perDeg, Y: r.Y + h*.72}, p.px(9))
	}
	// Where it's pointing, and the bearing under it.
	rl.DrawRectangleRec(rl.Rectangle{X: r.X + w/2 - max(1, p.px(1)), Y: r.Y + h - p.px(5), Width: max(2, p.px(2)), Height: p.px(9)}, colAccent)
	p.textIn(fmt.Sprintf("%03.0f", bearing), rl.Rectangle{X: r.X + w/2 - p.px(30), Y: r.Y + h + p.px(4), Width: p.px(60), Height: p.px(18)}, 13, semibold, colMuted, centre)
}

// fullMapRect is where the full map goes: most of the screen.
func fullMapRect(p painter, width, height float32) rl.Rectangle {
	return inset(rl.Rectangle{Width: width, Height: height}, p.px(48), p.px(48))
}

func drawFullMap(p painter, m *worldMap, at, facing rl.Vector2, width, height float32) {
	rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 160))
	r := fullMapRect(p, width, height)
	rl.DrawRectangleRec(inset(r, -p.px(8), -p.px(8)), colPanel)
	f := fullMapFrame(p, m, width, height)
	f.draw(m)
	f.drawRoads(max(2, p.px(2)))
	f.drawLabels(p, mapLabels{20, 14, 14})
	f.drawRoute(m, at, max(2, p.px(3)), p.px(16))
	if pos := f.toScreen(at); clip(rl.Rectangle{X: pos.X, Y: pos.Y, Width: 1, Height: 1}, r).Width > 0 {
		you(pos, f.screenDir(facing), p.px(7))
	}
	p.text("The Fringe", rl.Vector2{X: r.X + p.px(16), Y: r.Y + p.px(12)}, 30, black, colText)
	if m.marked {
		text := "Destination: " + distance(rl.Vector2Distance(at, m.dest))
		p.text(text, rl.Vector2{X: r.X + p.px(16), Y: r.Y + p.px(52)}, 16, semibold, colDest)
	}
	mark := "Mark"
	if m.marked {
		mark = "Mark, or take off"
	}
	x, y := r.X+p.px(16), r.Y+r.Height-p.px(14*1.7)-p.px(14)
	for _, k := range []struct{ key, does string }{{"M", "Close"}, {"Esc", "Close"}, {"Drag", "Move"}, {"Scroll", "Zoom"}, {"Click", mark}} {
		x += p.keycap(k.key, rl.Vector2{X: x, Y: y}, 14) + p.px(8)
		p.text(k.does, rl.Vector2{X: x, Y: y + p.px(4)}, 15, semibold, colText)
		x += p.measure(k.does, 15, semibold).X + p.px(20)
	}
}

// fullMapFrame is the full map's frame, north up.
func fullMapFrame(p painter, m *worldMap, width, height float32) mapFrame {
	return mapFrame{screen: fullMapRect(p, width, height), at: m.at, scale: m.zoom * p.s}
}

// Full map zoom, in pixels per metre at the UI's scale of 1.
const (
	mapZoomMin = .03 // the whole region
	mapZoomMax = 16
	mapZoomOut = .1 // where it opens: 10 km or so across
)

// mapState is mapInput's memory: whether the full map is open, and how
// far the pointer has moved since the left button went down (a click
// hardly moves; a drag does).
type mapState struct {
	opened   bool
	pressing bool
	moved    float32
}

// clickSlop is how far, in points, the pointer can move between the
// button going down and up for it to be a click, not a drag.
const clickSlop = 6

// mapInput moves the full map: drag to move it, scroll to zoom about the
// pointer, click to mark the destination or take it off. It opens on the
// player. In play, it takes the destination off when they get there.
func mapInput(
	m *illusion.Res[menu],
	wm *illusion.Res[worldMap],
	mouse *illusion.Res[input.Mouse],
	buttons *illusion.Res[input.MouseButtons],
	win *illusion.Res[window.Window],
	fonts *illusion.Res[uiFonts],
	ps *mapPlayers,
	state *illusion.Local[mapState],
) {
	wmap, ok := wm.TryGet()
	if !ok {
		return
	}
	at, _, _, here := player(&ps.roots, &ps.bodies, &ps.hier)
	if here && wmap.marked && rl.Vector2Distance(at, wmap.dest) < arrived {
		wmap.marked = false
	}
	st := state.Get()
	if m.Get().screen() != mapping {
		st.opened, st.pressing = false, false
		return
	}
	if !st.opened {
		st.opened = true
		if here {
			wmap.at = at
		}
		wmap.zoom = mapZoomOut
	}
	p := newPainter(fonts.Get(), win.Get())
	ms := mouse.Get()
	scale := wmap.zoom * p.s
	b := buttons.Get()
	if b.AnyPressed(rl.MouseButtonLeft, rl.MouseButtonRight) {
		wmap.at = rl.Vector2Subtract(wmap.at, rl.Vector2Scale(ms.Delta, 1/scale))
	}
	if b.JustPressed(rl.MouseButtonLeft) {
		st.pressing, st.moved = true, 0
	} else if st.pressing {
		st.moved += rl.Vector2Length(ms.Delta)
	}
	if b.JustReleased(rl.MouseButtonLeft) && st.pressing {
		st.pressing = false
		ww := win.Get()
		f := fullMapFrame(p, wmap, float32(ww.Width), float32(ww.Height))
		if st.moved <= p.px(clickSlop) && contains(f.screen, ms.Position) {
			click(wmap, f, ms.Position, p.px(markReach))
		}
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
	bounds := wmap.bounds
	wmap.at = rl.Vector2{X: min(bounds.X+bounds.Width, max(bounds.X, wmap.at.X)), Y: min(bounds.Y+bounds.Height, max(bounds.Y, wmap.at.Y))}
}

// click marks the destination where the full map, f, was clicked at s, or
// takes it off if the click was on its mark (within reach pixels).
func click(m *worldMap, f mapFrame, s rl.Vector2, reach float32) {
	if m.marked && rl.Vector2Distance(f.toScreen(m.dest), s) <= reach {
		m.marked = false
		return
	}
	d := f.toWorld(s)
	b := m.bounds
	m.dest = rl.Vector2{X: min(b.X+b.Width, max(b.X, d.X)), Y: min(b.Y+b.Height, max(b.Y, d.Y))}
	m.marked = true
}
