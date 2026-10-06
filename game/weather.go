package game

import (
	"math"
	"os"
	"strconv"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// The weather: dust, the Red's only weather (docs/look-and-feel.md). It's
// mostly clear. Now and then a dusty spell comes through, the air thick
// and brown and the far off gone; once in a while a dust storm, the light
// brown and close: the haze draws in to a couple of hundred metres, the
// sky and the sun go behind the dust, the sun dims and the shadows with
// it, and dust blows across the view. Under the dome the air's filtered,
// so the blown dust is mostly kept out, though the haze still closes in.
//
// The weather comes by the real clock, as everything in the shared world
// does (docs/shared-world.md), so everyone has the same: each weatherSlot
// may bring one spell (spells), at a time, for a length and as thick as
// its hash says. How thick it is, 0 to 1, is the weather's storm.
// EARTH_TWO_STORM, from 0 to 1, holds the weather that thick instead, for
// looking at it (go run ./tools/tour, say), as the test panel's Weather
// switch does.
const (
	weatherSlot = 3 * time.Hour
	// stormReach is how far the haze reaches (as shading.Haze's Distance)
	// in the hardest storm.
	stormReach = 70
	// stormEase is how quickly the weather follows a change it's told to
	// make at once (the forced storm's), the share 1 - e^-stormEase a second.
	stormEase = 1.5
	// dustyFrom and stormFrom are how thick the air is for it to be dusty,
	// and a storm.
	dustyFrom, stormFrom = .15, .6
)

// A spell of weather a slot may bring: what it's called, how likely it is,
// how long it lasts, how thick it gets at its height, and how long it
// takes to blow up and to die down.
type spell struct {
	name               string
	chance             float32
	shortest, longest  time.Duration
	thinnest, thickest float32
	rise               time.Duration
}

// spells are the slots' weather, by how likely: the rest of the time it's
// clear.
var spells = []spell{
	{"Dust storm", .06, 30 * time.Minute, 90 * time.Minute, .75, 1, 5 * time.Minute},
	{"Dusty", .2, 60 * time.Minute, 150 * time.Minute, .28, .45, 12 * time.Minute},
}

// What a storm turns things towards: the dust's brown, for the haze and
// the sky; the light through it, for the sun and the fill.
var (
	stormDust = rl.NewColor(150, 96, 64, 255)
	stormSun  = rl.NewColor(222, 150, 104, 255)
	stormFill = rl.NewColor(196, 138, 100, 255)
	// A storm by night: the dust dark, the haze it makes near black.
	nightDust = rl.NewColor(40, 30, 28, 255)
)

// weather is a resource: how thick the dust is, 0 to 1 (a storm from
// stormFrom), and what it was all made from (the clear day's haze, sun and fill), and the clock the
// blown dust is drawn by.
type weather struct {
	storm  float32
	forced float32 // a storm held (EARTH_TWO_STORM), or -1
	clear  struct {
		haze      shading.Haze
		ambient   render.AmbientLight
		sun       rl.Color
		brightest float32
		set       bool
	}
	t float32
}

func newWeather() *weather {
	w := &weather{forced: -1}
	if v, err := strconv.ParseFloat(os.Getenv("EARTH_TWO_STORM"), 32); err == nil {
		w.forced = clamp01(float32(v))
		w.storm = w.forced
	}
	return w
}

// stormAt is how thick the air is at t, 0 to 1, by the schedule.
func stormAt(t time.Time) float32 {
	slot := t.UnixNano() / int64(weatherSlot)
	// A spell late in the slot before may still be blowing.
	for _, n := range []int64{slot, slot - 1} {
		sp, start, length, peak, ok := spellIn(n)
		if !ok {
			continue
		}
		since, until := t.Sub(start), start.Add(length).Sub(t)
		if since < 0 || until < 0 {
			continue
		}
		rise := float32(sp.rise.Seconds())
		return peak * smoothstep(0, rise, float32(since.Seconds())) * smoothstep(0, rise, float32(until.Seconds()))
	}
	return 0
}

// spellIn is the spell of weather in a slot, if it has one: which, when it
// starts, how long it lasts and how thick it gets.
func spellIn(slot int64) (sp spell, start time.Time, length time.Duration, peak float32, ok bool) {
	h := func(k uint32) float32 { return lattice(int32(slot), int32(slot>>31), 0x5701+k) }
	pick := h(0)
	for _, s := range spells {
		if pick < s.chance {
			length = s.shortest + time.Duration(h(1)*float32(s.longest-s.shortest))
			start = time.Unix(0, slot*int64(weatherSlot)).Add(time.Duration(h(2) * float32(weatherSlot-length)))
			return s, start, length, s.thinnest + (s.thickest-s.thinnest)*h(3), true
		}
		pick -= s.chance
	}
	return spell{}, time.Time{}, 0, 0, false
}

// conditions is what the weather's called, as thick as storm is.
func conditions(storm float32) string {
	switch {
	case storm >= stormFrom:
		return "Dust storm"
	case storm >= dustyFrom:
		return "Dusty"
	}
	return "Clear"
}

// weatherPlugin is the storms: the weather as the clock has it, the light
// and the haze it makes, and the dust blown across the view.
type weatherPlugin struct{}

// dustSet draws the blown dust over the world and under the HUD.
const dustSet illusion.SystemSet = "game.dust"

func (weatherPlugin) Build(app *illusion.App) {
	app.InsertResource(illusion.R(newWeather()), illusion.R(newDaylight()))
	app.ConfigureSets(illusion.Render, dustSet.After(render.End2D).Before(render.Draw2D))
	// The sun where it is this hour (daylight.go), then the weather on it.
	app.AddSystems(illusion.Update, illusion.Chain(illusion.Fn5(turnSun), illusion.Fn6(blow)))
	app.AddSystems(illusion.Render, illusion.Fn4(drawDust).InSet(dustSet))
	// The stars, over the sky and behind the world (stars.go).
	app.ConfigureSets(illusion.Render, starSet.After(render.Draw3D).Before(render.End3D))
	app.AddSystems(illusion.Render, illusion.Fn4(drawStars).InSet(starSet))
	// The world's clock under the minimap (clock.go).
	app.AddSystems(illusion.Render, illusion.Fn5(drawClock).InSet(render.Draw2D))
	addTestSwitch(app, daylightSwitch())
}

// blow sets the weather by the clock (or the storm held), and the haze,
// the sun and the fill by it, on the light of the hour (daylight.go).
func blow(
	w *illusion.Res[weather],
	day *illusion.Res[daylight],
	haze *illusion.Res[shading.Haze],
	ambient *illusion.Res[render.AmbientLight],
	suns *illusion.Query1[render.DirectionalLight],
	clk *illusion.Res[illusion.Time],
) {
	wt, hz, am := w.Get(), haze.Get(), ambient.Get()
	dusk, night := day.Get().dusk, day.Get().night
	dt := clk.Get().DeltaSecs()
	wt.t += dt
	c := &wt.clear
	if !c.set {
		c.haze, c.ambient = *hz, *am
		c.sun, c.brightest = sunlight, sunBrightness
		c.set = true
	}
	want := stormAt(time.Now())
	if wt.forced >= 0 {
		want = wt.forced
	}
	// The schedule moves slowly enough as it is; a held storm comes in
	// over a second or two.
	wt.storm += (want - wt.storm) * float32(1-math.Exp(-stormEase*float64(dt)))
	s := wt.storm

	// The clear air at this hour: at dusk the haze takes the horizon's
	// rose, the fill goes violet (and stronger: the sky's the light then),
	// and the low sun deep orange and dimmer. At night it's dark: the haze
	// the night's horizon, a faint cold fill, and only the planets' light.
	clearHaze := mixColour(mixColour(c.haze.Color, duskHorizon, dusk), nightHorizon, night)
	clearFill := mixColour(mixColour(c.ambient.Color, duskFill, dusk), nightFill, night)
	fill := c.ambient.Brightness * (1 + .35*dusk) * (1 - .2*night)
	clearSun := mixColour(mixColour(c.sun, duskSun, dusk), planetLight, night)
	bright := c.brightest * (1 - .4*dusk) * (1 - .75*night)
	// And the storm on it, its dust as dark as the hour.
	dust := mixColour(stormDust, nightDust, night)
	hz.Color = mixColour(clearHaze, dust, s)
	// Drawn in towards stormReach, by the same share of the way each step.
	hz.Distance = c.haze.Distance * float32(math.Pow(float64(stormReach/c.haze.Distance), float64(s)))
	hz.Veil = .97 * smoothstep(0, .8, s)
	am.Color = mixColour(clearFill, mixColour(stormFill, nightFill, night), s)
	am.Brightness = fill * (1 + .7*s)
	suns.Each(func(_ ecs.Entity, l *render.DirectionalLight) {
		l.Color = mixColour(clearSun, stormSun, s)
		l.Brightness = bright * (1 - .8*s)
	})
}

// Blown dust: dustMotes motes blowing across the view on the wind, grit
// and the odd longer streak, nearer ones faster, longer and brighter;
// gusts, broad bands of thicker dust sweeping through; and over it all a
// brown cast. The wind blows from the
// east (from +X), so it crosses the view as the camera faces across it.
const (
	dustMotes = 420
	dustGusts = 6
	gustSteps = 8
)

var dustWind = rl.Vector3{X: -1, Z: .25}

// drawDust draws the blown dust, as hard as the storm and as much as the
// camera's out in it.
func drawDust(
	w *illusion.Res[weather],
	day *illusion.Res[daylight],
	win *illusion.Res[window.Window],
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
) {
	wt, ww := w.Get(), win.Get()
	// By night the dust's as dark as the night.
	dark := 1 - .8*day.Get().night
	dim := func(c rl.Color) rl.Color {
		return rl.NewColor(uint8(float32(c.R)*dark), uint8(float32(c.G)*dark), uint8(float32(c.B)*dark), c.A)
	}
	_, eye, _, ok := cameras.Single()
	if !ok || ww.Width == 0 || wt.storm < .02 {
		return
	}
	out := outdoors(eye.Translation)
	k := wt.storm * out
	if k < .02 {
		return
	}
	width, height := float32(rl.GetScreenWidth()), float32(rl.GetScreenHeight())
	right := rl.Vector3RotateByQuaternion(rl.Vector3{X: 1}, eye.Rotation)
	ahead := eye.Forward()
	wind := rl.Vector3Normalize(dustWind)
	// How the wind crosses the view: sideways across it, and how much it
	// blows into the camera's face (which reads as dust rushing past).
	across := rl.Vector3DotProduct(wind, right)
	into := -rl.Vector3DotProduct(wind, ahead)
	t := wt.t
	// The cast.
	rl.DrawRectangle(0, 0, int32(width), int32(height), dim(rl.NewColor(stormDust.R, stormDust.G, stormDust.B, uint8(70*k))))
	// The gusts: broad soft bands crossing the view.
	for i := range dustGusts {
		u := lattice(int32(i), 7, 0xd05)
		speed := (.25 + .35*u) * width * (across + .25*sign(across))
		bw := width * (.35 + .5*lattice(int32(i), 11, 0xd05))
		x := wrapf(u*(width+bw)+t*speed, width+bw) - bw
		y := height * (.15 + .7*lattice(int32(i), 13, 0xd05))
		bh := height * (.25 + .35*lattice(int32(i), 17, 0xd05))
		a := uint8(28 * k * (.5 + .5*float32(math.Sin(float64(t*.7+float32(i))))))
		// Thickest in its middle, in steps (the browser's raylib has no
		// gradients).
		for j := range gustSteps {
			f := (float32(j) + .5) / gustSteps
			c := dim(rl.NewColor(stormDust.R, stormDust.G, stormDust.B, uint8(float32(a)*(1-abs(2*f-1)))))
			rl.DrawRectangle(int32(x+bw*float32(j)/gustSteps), int32(y-bh/2), int32(bw/gustSteps)+1, int32(bh), c)
		}
	}
	// The motes: most short grit, some longer streaks, each slanting its
	// own way and wobbling on the turbulence, in the dust's own colours.
	n := int(float32(dustMotes) * k)
	for i := range n {
		depth := .2 + .8*lattice(int32(i), 1, 0xd17)
		u, v := lattice(int32(i), 2, 0xd17), lattice(int32(i), 3, 0xd17)
		long := lattice(int32(i), 4, 0xd17)
		slant := (lattice(int32(i), 5, 0xd17) - .35) * .5
		vx := width * (.5 + 1.1*depth) * (across + .15*sign(across))
		vy := vx*slant*.3 + height*.03*(1+depth)
		wob := float32(math.Sin(float64(t*(2+3*long)+float32(i)*1.7))) * 18 * depth
		x := u*width + t*vx
		y := v*height + t*vy + wob
		if into > 0 {
			// Blowing into the camera's face: out from the middle, growing.
			cx, cy := x-width/2, y-height/2
			grow := 1 + into*float32(math.Mod(float64(t*(.6+depth)+u*7), 1.5))
			x, y = width/2+cx*grow, height/2+cy*grow
		}
		x, y = wrapf(x, width), wrapf(y, height)
		streak := (2 + 6*depth) * (abs(across) + .3)
		if long > .8 {
			streak *= 4 + 4*depth
		}
		dx := streak * sign(vx) * (abs(across) + .15)
		dy := dx * slant
		shade := mixColour(rl.NewColor(170, 112, 76, 255), rl.NewColor(222, 176, 134, 255), lattice(int32(i), 6, 0xd17))
		shade.A = uint8((25 + 75*depth) * k)
		shade = dim(shade)
		// A thin rectangle along the streak (the browser's raylib has no
		// thick lines).
		thick := .8 + 1.6*depth
		rl.DrawRectanglePro(rl.Rectangle{X: x, Y: y, Width: float32(math.Hypot(float64(dx), float64(dy))) + 1, Height: thick},
			rl.Vector2{Y: thick / 2}, float32(math.Atan2(float64(-dy), float64(-dx)))*180/math.Pi, shade)
	}
}

// outdoors is how much of the blown dust reaches the camera at at: all of
// it outside, little under the dome, whose glass keeps it out.
func outdoors(at rl.Vector3) float32 {
	if at.Y < 30 && rectDistance(domeWalls, at.X, at.Z) == 0 {
		return .12
	}
	return 1
}

func sign(v float32) float32 {
	if v < 0 {
		return -1
	}
	return 1
}

// wrapf brings v into [0, n).
func wrapf(v, n float32) float32 {
	v = float32(math.Mod(float64(v), float64(n)))
	if v < 0 {
		v += n
	}
	return v
}
