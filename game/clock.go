package game

import (
	"fmt"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/window"
)

// The world's clock, under the minimap: Landfall time (UTC, as the world
// runs on the real clock), whether it's day, dawn, dusk or night, the
// weather, and the day as a strip: night dark, day light, dusk and dawn
// between, with now marked on it.
const (
	clockHeight = 50 // points
	clockGap    = 6  // between it and the minimap, and it and the map's key
)

func clockRect(p painter) rl.Rectangle {
	mini := minimapRect(p)
	border := max(1, p.px(3))
	return rl.Rectangle{X: mini.X - border, Y: mini.Y + mini.Height + border + p.px(clockGap), Width: mini.Width + 2*border, Height: p.px(clockHeight)}
}

// dayStrip is the day as the strip shows it: each hour's colour, from
// midnight.
var dayStrip = func() (out [24]rl.Color) {
	day := time.Date(2026, 10, 6, 0, 0, 0, 0, time.UTC)
	for h := range out {
		sun := sunAt(day.Add(time.Duration(h)*time.Hour + 30*time.Minute))
		c := mixColour(rl.NewColor(232, 196, 140, 255), rl.NewColor(214, 120, 120, 255), duskAt(sun))
		out[h] = mixColour(c, rl.NewColor(44, 42, 74, 255), nightAt(sun))
	}
	return out
}()

// partOfDay is what the time of day's called, with the sun the way sun at
// hour.
func partOfDay(sun rl.Vector3, hour float64) string {
	switch {
	case nightAt(sun) >= .5:
		return "Night"
	case duskAt(sun) >= .35 && hour < 12:
		return "Dawn"
	case duskAt(sun) >= .35:
		return "Dusk"
	}
	return "Day"
}

// drawClock draws the clock under the minimap, in play.
func drawClock(
	win *illusion.Res[window.Window],
	fonts *illusion.Res[uiFonts],
	m *illusion.Res[menu],
	day *illusion.Res[daylight],
	w *illusion.Res[weather],
) {
	if m.Get().screen() != playing {
		return
	}
	p := newPainter(fonts.Get(), win.Get())
	r := clockRect(p)
	rl.DrawRectangleRec(r, colPanel)
	now := day.Get().now()
	hour := hourOf(now)
	pad := p.px(10)
	// The time, large, and the part of the day and the weather beside it.
	p.textIn(fmt.Sprintf("%02d:%02d", now.Hour(), now.Minute()), rl.Rectangle{X: r.X + pad, Y: r.Y + p.px(4), Width: r.Width * .55, Height: r.Height - p.px(12)}, 24, semibold, colText, left)
	side := rl.Rectangle{X: r.X + r.Width*.5, Y: r.Y + p.px(5), Width: r.Width*.5 - pad, Height: p.px(17)}
	p.textIn(partOfDay(sunFrom, hour), side, 12, semibold, colText, right)
	side.Y += side.Height
	weatherNow := conditions(w.Get().storm)
	ink := colMuted
	switch weatherNow {
	case "Dusty":
		ink = colAccent
	case "Dust storm":
		ink = rl.NewColor(255, 120, 72, 255)
	}
	p.textIn(weatherNow, side, 12, semibold, ink, right)
	// The day, midnight to midnight, along the bottom, and now on it.
	strip := rl.Rectangle{X: r.X + pad, Y: r.Y + r.Height - p.px(8), Width: r.Width - 2*pad, Height: max(2, p.px(3))}
	per := strip.Width / 24
	for h, c := range dayStrip {
		rl.DrawRectangleRec(rl.Rectangle{X: strip.X + float32(h)*per, Y: strip.Y, Width: per + 1, Height: strip.Height}, c)
	}
	x := strip.X + float32(hour/24)*strip.Width
	rl.DrawRectangleRec(rl.Rectangle{X: x - max(1, p.px(1)), Y: strip.Y - p.px(3), Width: max(2, p.px(2)), Height: strip.Height + p.px(6)}, colText)
}
