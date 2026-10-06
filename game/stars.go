package game

import (
	"math"
	"sort"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// The night sky. The stars are points, not a picture of them: a fixed
// catalogue of starCount, most faint and a few bright, coloured by how hot
// they are (blue-white, white, yellow, orange, the odd red), thicker along
// the Milky Way, drawn a pixel each with the brightest given a faint
// sparkle. They wheel round the Red's pole through the night as it turns,
// twinkle (more low down, through more air), fade into the dust near the
// horizon, and go behind any dust in the air. The Milky Way itself, a faint
// band with dark lanes of dust down it and a brighter core, and the
// airglow low round the horizon, are painted into the sky (skyTexture),
// turned the same way.
const (
	starCount   = 9000
	starBand    = 5000 // more, faint, along the Milky Way
	starsRadius = 19700
	// The Red's pole is due north (-Z), poleUp above the horizon (as
	// Landfall's latitude would put it); the sky turns once a day about it.
	poleUp = 38 * math.Pi / 180
)

// star is one of the catalogue's: the way to it in the sky's own frame
// (turned to where it is now by skyTurn), how bright it is (0 to 1), its
// colour, and how it twinkles.
type star struct {
	dir    rl.Vector3
	bright float32
	colour rl.Color
	phase  float32
}

// The sky's own frame: the pole it turns about, and the Milky Way's plane
// (its normal) and its core, in it.
var (
	skyPole     = rl.Vector3{Y: float32(math.Sin(poleUp)), Z: -float32(math.Cos(poleUp))}
	galaxyPole  = rl.Vector3Normalize(rl.Vector3{X: .35, Y: .3, Z: .89})
	galaxyCore  = rl.Vector3Normalize(rl.Vector3CrossProduct(galaxyPole, rl.Vector3{Y: 1}))
	starClasses = []struct {
		share  float32
		colour rl.Color
	}{
		{.14, rl.NewColor(178, 200, 255, 255)}, // hot, blue-white
		{.4, rl.NewColor(236, 238, 255, 255)},  // white
		{.27, rl.NewColor(255, 240, 210, 255)}, // yellow
		{.15, rl.NewColor(255, 204, 150, 255)}, // orange
		{.04, rl.NewColor(255, 156, 120, 255)}, // red
	}
)

// stars is the catalogue: the same on every run.
var stars = func() []star {
	out := make([]star, 0, starCount+starBand)
	h := func(i int, k uint32) float32 { return lattice(int32(i), int32(k), 0x5ea7) }
	for i := range starCount + starBand {
		var dir rl.Vector3
		if i < starCount {
			// Even over the sphere.
			z, a := 2*h(i, 1)-1, 2*math.Pi*float64(h(i, 2))
			r := float32(math.Sqrt(float64(1 - z*z)))
			dir = rl.Vector3{X: r * float32(math.Cos(a)), Y: r * float32(math.Sin(a)), Z: z}
		} else {
			// Close about the Milky Way's plane.
			a := 2 * math.Pi * float64(h(i, 1))
			off := (h(i, 2) - .5) * .25
			across := rl.Vector3Normalize(rl.Vector3CrossProduct(galaxyPole, galaxyCore))
			in := rl.Vector3Add(rl.Vector3Scale(galaxyCore, float32(math.Cos(a))), rl.Vector3Scale(across, float32(math.Sin(a))))
			dir = rl.Vector3Normalize(rl.Vector3Add(in, rl.Vector3Scale(galaxyPole, off)))
		}
		// Most faint: brightness falls away as a power, as magnitudes do.
		b := float32(math.Pow(float64(h(i, 3)), 6))
		if i >= starCount {
			b *= .4
		}
		pick, colour := h(i, 4), starClasses[1].colour
		for _, c := range starClasses {
			if pick < c.share {
				colour = c.colour
				break
			}
			pick -= c.share
		}
		out = append(out, star{dir: dir, bright: .22 + .78*b, colour: colour, phase: 2 * math.Pi * h(i, 5)})
	}
	// Brightest first, so a budget (starBudget) keeps the ones that show.
	sort.SliceStable(out, func(i, j int) bool { return out[i].bright > out[j].bright })
	return out
}()

// skyTurn is how far the sky's turned about its pole at t: once a day.
func skyTurn(t time.Time) float32 { return float32(2 * math.Pi * hourOf(t) / 24) }

// skyFrame turns a way in the sky's own frame to where it is in the sky,
// turned by turn.
func skyFrame(turn float32) rl.Quaternion {
	return rl.QuaternionFromAxisAngle(skyPole, -turn)
}

// milkyWay is how bright the Milky Way is the way dir (in the sky's own
// frame), 0 to about 1: a band along its plane, brighter and wider towards
// its core, mottled, with dark lanes of dust down its middle.
func milkyWay(dir rl.Vector3) float32 {
	off := rl.Vector3DotProduct(dir, galaxyPole)
	core := (1 + rl.Vector3DotProduct(dir, galaxyCore)) / 2
	width := .11 + .12*core*core
	band := float32(math.Exp(-float64(off*off) / float64(width*width)))
	mottle := .55 + .45*fbm(dir.X*5+dir.Z*3+11, dir.Y*5-dir.Z*2+7)
	lane := 1 - .75*smoothstep(.55, .8, fbm(dir.X*9-3, dir.Y*9+dir.Z*6))*float32(math.Exp(-float64(off*off)/float64(width*width*.12)))
	return band * mottle * lane * (.35 + .65*core*core)
}

// nightSky is the night's own light the way dir in the sky: the Milky
// Way, turned by turn, and the airglow low round the horizon, added to the
// sky colour c as night of the way into night.
func nightSky(c rl.Color, dir rl.Vector3, turn, night float32) rl.Color {
	if night <= 0 {
		return c
	}
	up := float32(math.Asin(float64(dir.Y)))
	own := rl.Vector3RotateByQuaternion(dir, rl.QuaternionInvert(skyFrame(turn)))
	// Dimmed through the dust low down.
	clearAir := smoothstep(0, .35, up)
	mw := milkyWay(own) * clearAir * night
	glow := night * smoothstep(.3, 0, up) * smoothstep(-.05, .02, up)
	add := func(v uint8, by float32) uint8 { return uint8(min(255, float32(v)+by)) }
	return rl.NewColor(
		add(c.R, 92*mw+26*glow),
		add(c.G, 86*mw+14*glow),
		add(c.B, 108*mw+12*glow),
		255)
}

// starSet draws the stars: after the world's drawn (so the ground and the
// planets hide those behind them) and before 3D drawing ends.
const starSet illusion.SystemSet = "game.stars"

// drawStars draws the stars as many as it's night, and as the air's clear.
func drawStars(
	day *illusion.Res[daylight],
	haze *illusion.Res[shading.Haze],
	w *illusion.Res[weather],
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
) {
	d := day.Get()
	if d.night < .01 {
		return
	}
	_, eye, camera, ok := cameras.Single()
	if !ok {
		return
	}
	// Any dust in the air hides them: the veil in a storm, and the
	// thickening in a dusty spell.
	clearAir := (1 - haze.Get().Veil) * (1 - smoothstep(0, stormFrom, w.Get().storm))
	k := d.night * clearAir
	if k < .01 {
		return
	}
	turn := skyFrame(skyTurn(d.now()))
	t := w.Get().t
	up := rl.Vector3RotateByQuaternion(transform.Up, eye.Rotation)
	right := rl.Vector3RotateByQuaternion(rl.Vector3{X: 1}, eye.Rotation)
	// The catalogue covers the whole sky. Reject stars outside the camera
	// before issuing draw calls (each call crosses into JavaScript on web).
	fovy := camera.Fovy
	if fovy == 0 {
		fovy = 45
	}
	v := view{ahead: eye.Forward(), up: up, right: right,
		tanV:   float32(math.Tan(float64(fovy) * math.Pi / 360)),
		aspect: float32(rl.GetScreenWidth()) / max(1, float32(rl.GetScreenHeight()))}
	v.prepare()
	// A dot's size: about a pixel, at their distance.
	px := float32(starsRadius) * .00055
	for i := range min(len(stars), starBudget) {
		s := &stars[i]
		dir := rl.Vector3RotateByQuaternion(s.dir, turn)
		if dir.Y < -.02 {
			continue
		}
		if !v.sees(rl.Vector3Scale(dir, starsRadius), px*5, starsRadius+px*5) {
			continue
		}
		low := smoothstep(.02, .3, dir.Y)
		twinkle := 1 - (.12+.3*(1-low))*(.5+.5*float32(math.Sin(float64(t*(2+3*s.phase)+s.phase*7))))
		a := k * (.3 + .7*s.bright) * twinkle * (.15 + .85*low)
		if a < .03 {
			continue
		}
		at := rl.Vector3Add(eye.Translation, rl.Vector3Scale(dir, starsRadius))
		c := s.colour
		c.A = uint8(255 * min(1, a))
		size := px * (.7 + 1.8*s.bright*s.bright)
		rl.DrawLine3D(rl.Vector3Subtract(at, rl.Vector3Scale(right, size)), rl.Vector3Add(at, rl.Vector3Scale(right, size)), c)
		if !starsCrossed {
			continue
		}
		rl.DrawLine3D(rl.Vector3Subtract(at, rl.Vector3Scale(up, size)), rl.Vector3Add(at, rl.Vector3Scale(up, size)), c)
		if s.bright > .75 {
			// The brightest: a faint sparkle.
			spike := px * 5 * s.bright
			c.A = uint8(float32(c.A) * .22)
			rl.DrawLine3D(rl.Vector3Subtract(at, rl.Vector3Scale(right, spike)), rl.Vector3Add(at, rl.Vector3Scale(right, spike)), c)
			rl.DrawLine3D(rl.Vector3Subtract(at, rl.Vector3Scale(up, spike)), rl.Vector3Add(at, rl.Vector3Scale(up, spike)), c)
		}
	}
}
