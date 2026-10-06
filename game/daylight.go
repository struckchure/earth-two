package game

import (
	"math"
	"os"
	"strconv"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// The time of day, on the real clock as the world runs (Landfall time is
// UTC). The sun rises about 05:00, crosses the south low at noon and sets
// about 19:00 (sky.go's sunAt). As it nears the horizon the light goes the
// way docs/look-and-feel.md has it at dusk: the sky violet overhead and
// rose at the horizon, the sun deep orange and dimmer, the shadows long;
// and dawn the same. At night the sky's dark and starred, the only light
// outside a faint cold one off the neighbour planets, and the lamps and
// beacons light the town. The weather's storms blow on top of whatever the
// hour is.
//
// EARTH_TWO_HOUR (0 to 24) holds the sun where it is at that hour, for
// looking at it (with the tour, say), and the test panel's Time of day
// switch does the same.

// daylight is a resource: the hour the sun's held at (-1 to follow the
// clock), how much of dusk's colour there is and how far into night it is,
// and the sun, dusk and night the sky was last painted for.
type daylight struct {
	hour        float32
	dusk, night float32
	baked       skyPaint
	// painting is the sky being painted off the main thread, if it is:
	// what for, and where its pixels come.
	painting *skyPaint
	painted  chan []byte
}

// skyPaint is what the sky's painted for: the sun's way, dusk, night, and
// how far the stars' sky has turned.
type skyPaint struct {
	dir               rl.Vector3
	dusk, night, turn float32
	done              bool
}

// skyRepaint is how far the sun moves, or the stars' sky turns (radians),
// or dusk or night comes on, before the sky's painted again: the glare
// round the sun and the Milky Way are in its texture.
const skyRepaint, duskRepaint = .4 * math.Pi / 180, .03

// stale reports whether the sky painted for b wants painting again for
// now.
func (b skyPaint) stale(now skyPaint) bool {
	switch {
	case !b.done:
		return true
	case abs(b.dusk-now.dusk) >= duskRepaint || abs(b.night-now.night) >= duskRepaint:
		return true
	// Once the sun's well down its glare's gone, so its moving doesn't
	// show; the Milky Way's turning does.
	case now.night < .99 && math.Acos(math.Min(1, float64(rl.Vector3DotProduct(b.dir, now.dir)))) > skyRepaint:
		return true
	case now.night > .01 && abs(b.turn-now.turn) > skyRepaint:
		return true
	}
	return false
}

func newDaylight() *daylight {
	d := &daylight{hour: -1}
	if v, err := strconv.ParseFloat(os.Getenv("EARTH_TWO_HOUR"), 32); err == nil {
		d.hour = float32(math.Mod(math.Max(v, 0), 24))
	}
	sun := sunAt(d.now())
	d.dusk, d.night = duskAt(sun), nightAt(sun)
	return d
}

// now is when the sun's at: the clock's time, or the held hour today.
func (d *daylight) now() time.Time {
	t := time.Now().UTC()
	if d.hour < 0 {
		return t
	}
	day := time.Date(t.Year(), t.Month(), t.Day(), 0, 0, 0, 0, time.UTC)
	return day.Add(time.Duration(float64(d.hour) * float64(time.Hour)))
}

// turnSun puts the sun where it is now: the light's way, the sun in the
// sky (moveSky follows sunFrom), and the sky painted again round it once
// it's moved on, or the stars' sky has turned, or dusk or night's come on,
// enough to see.
func turnSun(
	day *illusion.Res[daylight],
	suns *illusion.Query2[transform.Transform, render.DirectionalLight],
	domes *illusion.Query2[render.MeshMaterial3d, skyBody],
	materials *illusion.Res[asset.Assets[render.StandardMaterial]],
	textures *illusion.Res[asset.Assets[render.Texture]],
) {
	d := day.Get()
	sunFrom = sunAt(d.now())
	d.dusk, d.night = duskAt(sunFrom), nightAt(sunFrom)
	light := lightFrom(sunFrom, d.night)
	suns.Each(func(_ ecs.Entity, tr *transform.Transform, _ *render.DirectionalLight) {
		*tr = transform.FromTranslation(tr.Translation).LookingAt(rl.Vector3Add(tr.Translation, rl.Vector3Negate(light)), transform.Up)
	})
	now := skyPaint{dir: sunFrom, dusk: d.dusk, night: d.night, turn: skyTurn(d.now()), done: true}
	// A sky painted off the main thread: put it up.
	if d.painting != nil {
		select {
		case pixels := <-d.painted:
			domes.Each(func(_ ecs.Entity, m *render.MeshMaterial3d, s *skyBody) {
				if !s.dome {
					return
				}
				if mat := materials.Get().Get(m.Material); mat != nil {
					old := mat.Texture
					mat.Texture = textures.Get().Add(uploadTexture(pixels, skyTexW, skyTexH))
					textures.Get().Remove(old)
				}
			})
			d.baked, d.painting = *d.painting, nil
		default:
			return // still painting
		}
	}
	if !d.baked.stale(now) || skyMesh.VertexCount == 0 {
		return
	}
	// Paint it again, off the main thread, so the frame doesn't wait.
	d.painting = &now
	if d.painted == nil {
		d.painted = make(chan []byte, 1)
	}
	go func(p skyPaint) { d.painted <- paintSky(skyMesh, p.dir, p.dusk, p.night, p.turn) }(now)
}

// lightFrom is the way the scene's light comes from, with the sun the way
// sun and night of the way into night: the sun's, until it's down, and then
// the planets', from above where the sun went (so they show it lit on the
// side towards it), never from under the ground.
func lightFrom(sun rl.Vector3, night float32) rl.Vector3 {
	flat := rl.Vector3Normalize(rl.Vector3{X: sun.X, Z: sun.Z})
	up := float32(math.Max(math.Asin(float64(sun.Y)), 25*math.Pi/180*float64(night)))
	up = max(up, 3*math.Pi/180)
	return rl.Vector3Add(rl.Vector3Scale(flat, float32(math.Cos(float64(up)))), rl.Vector3{Y: float32(math.Sin(float64(up)))})
}

// daylightHours are the test panel's settings: the clock, or the sun held
// in the day, at dusk, in the night, or at dawn.
var daylightHours = []struct {
	name string
	hour float32
}{{"Real clock", -1}, {"Day", 11}, {"Dusk", 18.5}, {"Night", 0}, {"Dawn", 5.5}}

// daylightSwitch is the test panel's Time of day.
func daylightSwitch() testSwitch {
	names := make([]string, len(daylightHours))
	for i, h := range daylightHours {
		names[i] = h.name
	}
	return testSwitch{
		Name:     "Time of day",
		Settings: names,
		Get: func(w *ecs.World) int {
			if d := ecs.GetResource[daylight](w); d != nil {
				for i, h := range daylightHours {
					if h.hour == d.hour {
						return i
					}
				}
			}
			return 0
		},
		Set: func(w *ecs.World, i int) {
			if d := ecs.GetResource[daylight](w); d != nil {
				d.hour = daylightHours[i].hour
			}
		},
	}
}
