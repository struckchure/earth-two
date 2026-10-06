package game

import (
	"math"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// The sky: where the Red is. It's TRAPPIST-1e, the fourth of seven planets
// round TRAPPIST-1, a red dwarf 40.7 light-years away in Aquarius (right
// ascension 23h 06m 29s, declination −5° 02′). It's one of the most
// Earth-like planets known: 0.92 of Earth's radius, 0.69 of its mass, in
// the star's habitable zone. (No planet like Earth is known outside the
// Milky Way; the nearest the search gets, M51-ULS-1b in the Whirlpool
// galaxy, is an unconfirmed giant seen by X-rays.)
//
// The star is small and cool, 2,566 K, but so close, 0.029 AU, that it's
// 2.2° across from here, four times the Sun from Earth, and orange-red. The
// other six planets are so near that several show discs the size of the
// Moon, and wander along the ecliptic, the band the sun sits on, taking
// one to three weeks to come round again. They're placed from their real
// orbits (Agol et al. 2021) at the real time, and drawn ten times their
// size: true to life, the biggest is a few pixels across, and they're there
// to say this isn't Earth. They keep their sizes to each other, and the sun
// is its real size.
//
// The Red is likely tidally locked, one face always to its star, so the
// sun keeps its place in the sky; Landfall is towards the edge of the lit
// side, where it hangs low. The sky follows the camera: it's drawn as if
// infinitely far away.

// A planet of TRAPPIST-1: its letter, how far it orbits (AU), how long it
// takes (days), its radius (Earth radii), and where it was in its orbit
// (degrees from the Red's, as the sun sees it) at skyEpoch. How it looks is
// in skytex.go.
type exoplanet struct {
	name     string
	a, p, r  float64
	startDeg float64
}

var (
	// theRed is TRAPPIST-1e.
	theRed = exoplanet{name: "e", a: 0.02925, p: 6.101, r: 0.920}
	// Its neighbours. Where they start is ours to say: nobody knows.
	neighbours = []exoplanet{
		{"b", 0.01154, 1.511, 1.116, 200}, // too hot for water: Venus-like
		{"c", 0.01580, 2.422, 1.097, 290},
		{"d", 0.02227, 4.049, 0.788, 110}, // maybe an ocean world
		{"f", 0.03849, 9.208, 1.045, 70},  // past the snow line: ice, maybe water
		{"g", 0.04683, 12.352, 1.129, 150},
		{"h", 0.06189, 18.773, 0.755, 250},
	}
	skyEpoch = time.Date(2026, 10, 5, 0, 0, 0, 0, time.UTC)
)

const (
	auKm         = 1.496e8
	earthKm      = 6371.0
	sunKm        = 0.1192 * 696000 // TRAPPIST-1's radius
	planetScale  = 10              // how much bigger the planets are drawn than they look
	skyRadius    = 900             // the dome, inside the camera's far plane
	sunDistance  = 820
	bodyDistance = 760
)

// Where the sun is, and so the way the scene's light comes from (see
// setup): sunUp above the horizon, off to the east-south-east.
const sunUp = 20 * math.Pi / 180

var sunFrom = func() rl.Vector3 {
	across := rl.Vector3Normalize(rl.Vector3{X: 2, Z: 1})
	return rl.Vector3Add(rl.Vector3Scale(across, float32(math.Cos(sunUp))), rl.Vector3{Y: float32(math.Sin(sunUp))})
}()

// eclipticTilt is how high the ecliptic rises above the horizon: the
// planets wander no higher, where the camera can look.
const eclipticTilt = 32 * math.Pi / 180

// The sky's colours: overhead, and at the horizon, thin dusty air lit by a
// red sun (docs/look-and-feel.md: pale ochre by day).
var (
	skyTop     = rl.NewColor(190, 136, 98, 255)
	skyHorizon = rl.NewColor(246, 204, 150, 255)
	// The glare round the sun, and the light it gives: a red dwarf's, warm
	// orange.
	glareColour = rl.NewColor(255, 238, 200, 255)
	sunlight    = rl.NewColor(255, 182, 128, 255)
)

// sunBrightness scales sunlight: past white on what faces the sun, so the
// low sun reads as bright against the shadows.
const sunBrightness = 1.3

// skyBody is a part of the sky, and where it is from the camera: dir a
// unit direction, at a distance, scaled to look size across.
type skyBody struct {
	dir      rl.Vector3
	distance float32
	size     float32
	dome     bool // the dome: centred on the camera, not off in a direction
	// facing turns its top pole to the camera (the sun, whose face is
	// painted from the middle of its disc out); spin turns it about the
	// ecliptic's pole, by this angle (a planet).
	facing bool
	spin   float32
}

// ecliptic is the band the sun and planets move on, as two directions in
// it: the sun, s, and v a quarter of the way round from it. It runs through
// the sun and rises no higher than eclipticTilt.
func ecliptic() (s, v rl.Vector3) {
	s = sunFrom
	// Its normal leans eclipticTilt off the vertical, the way that keeps
	// the sun on it.
	flat := rl.Vector3Normalize(rl.Vector3{X: s.X, Z: s.Z})
	side := rl.Vector3{X: -flat.Z, Z: flat.X}
	cosPhi := -math.Tan(sunUp) / math.Tan(eclipticTilt)
	phi := math.Acos(max(-1, min(1, cosPhi)))
	k := rl.Vector3Add(rl.Vector3Scale(flat, float32(math.Cos(phi))), rl.Vector3Scale(side, float32(math.Sin(phi))))
	n := rl.Vector3Add(rl.Vector3Scale(transform.Up, float32(math.Cos(eclipticTilt))), rl.Vector3Scale(k, float32(math.Sin(eclipticTilt))))
	return s, rl.Vector3Normalize(rl.Vector3CrossProduct(n, s))
}

// neighbourAt is where a neighbour is in the sky days after skyEpoch: the
// way to it, and its angular radius (radians), drawn size and all.
func neighbourAt(n exoplanet, days float64) (rl.Vector3, float64) {
	// In the Red's frame, which turns with it: the sun is off along -X.
	turn := (n.startDeg*math.Pi/180 + 2*math.Pi*days*(1/n.p-1/theRed.p))
	rx, ry := n.a*math.Cos(turn)-theRed.a, n.a*math.Sin(turn)
	dist := math.Hypot(rx, ry) * auKm
	s, v := ecliptic()
	dir := rl.Vector3Normalize(rl.Vector3Add(rl.Vector3Scale(s, float32(-rx)), rl.Vector3Scale(v, float32(ry))))
	return dir, math.Asin(min(1, n.r*earthKm/dist)) * planetScale
}

// sunRadius is the sun's angular radius, in radians.
func sunRadius() float64 { return math.Asin(sunKm / (theRed.a * auKm)) }

// skyDays is how many days it is since skyEpoch, on the real clock: the
// world runs in real time (docs/decisions.md).
func skyDays() float64 { return time.Since(skyEpoch).Hours() / 24 }

// planetSky marks a neighbour in the sky.
type planetSky struct{ i int }

// spawnSky adds the sky: the dome, the sun and its glow, and the planets.
// None of it casts shadows: it's all far beyond what it would shade.
func spawnSky(cmd *illusion.Commands, meshes *asset.Assets[render.Mesh], materials *asset.Assets[render.StandardMaterial], textures *asset.Assets[render.Texture]) {
	ball := render.Sphere(1)
	sphere := meshes.Add(ball)
	unlit := func(c rl.Color, tex asset.Handle[render.Texture]) asset.Handle[render.StandardMaterial] {
		return materials.Add(render.StandardMaterial{BaseColor: c, Texture: tex, Unlit: true})
	}
	// The dome, turned inside out (scaled by -1 across) so it's seen from
	// within, with the gradient down it.
	cmd.Spawn(
		illusion.C(render.Mesh3d{Mesh: sphere}),
		illusion.C(render.MeshMaterial3d{Material: unlit(rl.White, textures.Add(skyTexture(ball.Mesh)))}),
		illusion.C(transform.Identity()),
		illusion.C(skyBody{dome: true, size: skyRadius}),
		illusion.C(render.NotShadowCaster{}),
	)
	// The sun: a sphere with its face painted on (skytex.go), its top pole
	// turned to the camera. Its glare is painted into the dome behind it.
	r := float32(sunRadius())
	cmd.Spawn(
		illusion.C(render.Mesh3d{Mesh: sphere}),
		illusion.C(render.MeshMaterial3d{Material: unlit(rl.White, textures.Add(sphereTexture(ball.Mesh, sunSurface)))}),
		illusion.C(transform.Identity()),
		illusion.C(skyBody{dir: sunFrom, distance: sunDistance, size: sunDistance * float32(math.Tan(float64(r))), facing: true}),
		illusion.C(render.NotShadowCaster{}),
	)
	// The neighbours, their surfaces painted on (skytex.go) and lit by the
	// scene's light, so they show phases.
	for i, n := range neighbours {
		cmd.Spawn(
			illusion.C(render.Mesh3d{Mesh: sphere}),
			illusion.C(render.MeshMaterial3d{Material: materials.Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.Add(sphereTexture(ball.Mesh, surfaces[n.name]))})}),
			illusion.C(transform.Identity()),
			illusion.C(skyBody{distance: bodyDistance}),
			illusion.C(planetSky{i}),
			illusion.C(render.NotShadowCaster{}),
		)
	}
}

// moveSky keeps the sky round the camera, and the planets where they are
// now.
func moveSky(
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
	bodies *illusion.Query2[transform.Transform, skyBody],
	planets *illusion.Query2[skyBody, planetSky],
) {
	_, eye, _, ok := cameras.Single()
	if !ok {
		return
	}
	days := skyDays()
	planets.Each(func(_ ecs.Entity, b *skyBody, p *planetSky) {
		n := neighbours[p.i]
		dir, radius := neighbourAt(n, days)
		b.dir, b.size = dir, b.distance*float32(math.Tan(radius))
		// Each turns once an orbit, held face-on to its star like the Red.
		b.spin = float32(2 * math.Pi * math.Mod(days/n.p, 1))
	})
	s, v := ecliptic()
	pole := rl.Vector3CrossProduct(s, v)
	bodies.Each(func(_ ecs.Entity, tr *transform.Transform, b *skyBody) {
		if b.dome {
			tr.Translation = eye.Translation
			tr.Scale = rl.Vector3{X: -b.size, Y: b.size, Z: b.size}
			return
		}
		tr.Translation = rl.Vector3Add(eye.Translation, rl.Vector3Scale(b.dir, b.distance))
		tr.Scale = rl.Vector3{X: b.size, Y: b.size, Z: b.size}
		switch {
		case b.facing:
			tr.Rotation = rl.QuaternionFromVector3ToVector3(transform.Up, rl.Vector3Negate(b.dir))
		default:
			// Upright on the ecliptic's pole, turned by its spin.
			upright := rl.QuaternionFromVector3ToVector3(transform.Up, pole)
			tr.Rotation = rl.QuaternionMultiply(rl.QuaternionFromAxisAngle(pole, b.spin), upright)
		}
	})
}

// skyColour is the sky, without the sun, at elevation up (radians):
// overhead down to the horizon, and the horizon's colour below.
func skyColour(up float64) rl.Color {
	t := clamp01(float32(1 - up/(math.Pi/2)))
	return mixColour(skyTop, skyHorizon, float32(math.Pow(float64(t), 2.2)))
}

// glare is how much of the sun's glare there is off from it by off
// (radians): dense close in, thinning out, and a wide warmth round it.
func glare(off float64) float32 {
	deg := off * 180 / math.Pi
	return clamp01(float32(.95*math.Exp(-deg/1.8) + .5*math.Exp(-deg/7) + .18*math.Exp(-deg/30)))
}

// skyTexture is the dome's texture, for raylib's sphere mesh m (which the
// dome is, turned inside out): the sky's colour by elevation, and the
// sun's glare round where it hangs, which it never leaves.
func skyTexture(m rl.Mesh) render.Texture {
	return uploadTexture(paintMesh(m, skyTexW, skyTexH, func(p rl.Vector3) rl.Color {
		// The dome's scaled by -1 across, so a point's X is flipped.
		dir := rl.Vector3{X: -p.X, Y: p.Y, Z: p.Z}
		off := math.Acos(math.Max(-1, math.Min(1, float64(rl.Vector3DotProduct(dir, sunFrom)))))
		return mixColour(skyColour(math.Asin(float64(dir.Y))), glareColour, glare(off))
	}), skyTexW, skyTexH)
}

// mixColour is a share t of the way from a to b.
func mixColour(a, b rl.Color, t float32) rl.Color {
	m := func(x, y uint8) uint8 { return uint8(float32(x) + (float32(y)-float32(x))*t) }
	return rl.NewColor(m(a.R, b.R), m(a.G, b.G), m(a.B, b.B), 255)
}
