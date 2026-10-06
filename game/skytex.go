package game

import (
	"math"
	"runtime"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
)

// The sky's surfaces: textures painted here, for raylib's sphere, from
// 3D noise on the sphere, so they don't seam where they wrap round. Nobody
// knows what TRAPPIST-1's planets look like; these are guesses that go
// with what they're thought to be (sky.go).

// surface is how a body looks at a point p on its unit sphere.
type surface func(p rl.Vector3) rl.Color

// Texture sizes.
const skyTexW, skyTexH = 512, 256

// sphereTexture paints s onto a texture for the sphere mesh m (see
// paintMesh).
func sphereTexture(m rl.Mesh, s surface) render.Texture {
	return uploadTexture(paintMesh(m, skyTexW, skyTexH, s), skyTexW, skyTexH)
}

// paintMesh paints what f says each point of mesh m looks like (f is
// given the point's direction from the mesh's middle) into a w by h
// texture, through the mesh's own texture coordinates: each triangle's
// patch of texture is filled with what's under it. raylib's sphere doesn't
// wrap its texture round the usual way, so this doesn't assume one.
func paintMesh(m rl.Mesh, w, h int, f surface) []byte {
	n := int(m.VertexCount)
	pos := unsafe.Slice(m.Vertices, 3*n)
	uv := unsafe.Slice(m.Texcoords, 2*n)
	var tris []int
	if m.Indices != nil {
		for _, i := range unsafe.Slice(m.Indices, 3*int(m.TriangleCount)) {
			tris = append(tris, int(i))
		}
	} else {
		for i := range n {
			tris = append(tris, i)
		}
	}
	pixels := make([]byte, w*h*4)
	painted := make([]bool, w*h)
	for t := 0; t+2 < len(tris); t += 3 {
		var px, py [3]float32
		var p [3]rl.Vector3
		for k := range 3 {
			i := tris[t+k]
			px[k], py[k] = uv[2*i]*float32(w)-.5, uv[2*i+1]*float32(h)-.5
			p[k] = rl.Vector3{X: pos[3*i], Y: pos[3*i+1], Z: pos[3*i+2]}
		}
		area := (px[1]-px[0])*(py[2]-py[0]) - (px[2]-px[0])*(py[1]-py[0])
		if abs(area) < 1e-6 {
			continue
		}
		x0, x1 := int(max(0, min(px[0], px[1], px[2])-1)), int(min(float32(w-1), max(px[0], px[1], px[2])+1))
		y0, y1 := int(max(0, min(py[0], py[1], py[2])-1)), int(min(float32(h-1), max(py[0], py[1], py[2])+1))
		for y := y0; y <= y1; y++ {
			for x := x0; x <= x1; x++ {
				fx, fy := float32(x), float32(y)
				b1 := ((fx-px[0])*(py[2]-py[0]) - (px[2]-px[0])*(fy-py[0])) / area
				b2 := ((px[1]-px[0])*(fy-py[0]) - (fx-px[0])*(py[1]-py[0])) / area
				b0 := 1 - b1 - b2
				const e = -.02
				if b0 < e || b1 < e || b2 < e {
					continue
				}
				d := rl.Vector3Normalize(rl.Vector3Add(rl.Vector3Add(rl.Vector3Scale(p[0], b0), rl.Vector3Scale(p[1], b1)), rl.Vector3Scale(p[2], b2)))
				c := f(d)
				copy(pixels[(y*w+x)*4:], []byte{c.R, c.G, c.B, c.A})
				painted[y*w+x] = true
			}
		}
	}
	// Anything the triangles missed takes its nearest painted neighbour
	// along its row, so no gaps show at seams.
	for y := range h {
		for x := range w {
			if painted[y*w+x] {
				continue
			}
			for d := 1; d < w; d++ {
				if l := x - d; l >= 0 && painted[y*w+l] {
					copy(pixels[(y*w+x)*4:(y*w+x)*4+4], pixels[(y*w+l)*4:])
					break
				}
				if r := x + d; r < w && painted[y*w+r] {
					copy(pixels[(y*w+x)*4:(y*w+x)*4+4], pixels[(y*w+r)*4:])
					break
				}
			}
		}
	}
	return pixels
}

// uploadTexture makes a texture of w by h RGBA pixels.
func uploadTexture(pixels []byte, w, h int) render.Texture {
	img := rl.NewImage(pixels, int32(w), int32(h), 1, rl.UncompressedR8g8b8a8)
	tex := rl.LoadTextureFromImage(img)
	runtime.KeepAlive(pixels) // the image is ours, not raylib's: it's never unloaded
	rl.SetTextureFilter(tex, rl.FilterBilinear)
	return render.Texture{Texture2D: tex}
}

// The neighbours' surfaces, by letter.
var surfaces = map[string]surface{
	// b: too hot for water, under thick cloud: Venus-like, banded and swirled.
	"b": func(p rl.Vector3) rl.Color {
		warp := fbm3(rl.Vector3Scale(p, 3), 11)
		band := .5 + .5*math.Sin(float64(p.Y+.35*warp)*13)
		c := mixColour(rl.NewColor(206, 156, 124, 255), rl.NewColor(240, 220, 178, 255), float32(band))
		return brighten(c, .85+.3*fbm3(rl.Vector3Scale(p, 9), 12))
	},
	// c: bare rock, tan with dark plains and pocked with craters.
	"c": func(p rl.Vector3) rl.Color {
		c := rl.NewColor(178, 142, 106, 255)
		plains := smoothstep(.5, .62, fbm3(rl.Vector3Scale(p, 2.5), 21))
		c = mixColour(c, rl.NewColor(104, 82, 66, 255), plains*.85)
		pits := smoothstep(.66, .72, fbm3(rl.Vector3Scale(p, 14), 22))
		return brighten(c, (1-.35*pits)*(.85+.3*fbm3(rl.Vector3Scale(p, 6), 23)))
	},
	// d: maybe an ocean world: deep water under swirls of cloud.
	"d": func(p rl.Vector3) rl.Color {
		sea := mixColour(rl.NewColor(22, 58, 112, 255), rl.NewColor(44, 104, 160, 255), fbm3(rl.Vector3Scale(p, 3), 31))
		warp := rl.Vector3Add(rl.Vector3Scale(p, 4), rl.Vector3{X: 2 * fbm3(rl.Vector3Scale(p, 2), 32)})
		cloud := smoothstep(.52, .66, fbm3(warp, 33))
		return mixColour(sea, rl.NewColor(244, 246, 250, 255), cloud*.9)
	},
	// f: past the snow line: water, ice caps reaching down from the poles,
	// thin streaks of cloud.
	"f": func(p rl.Vector3) rl.Color {
		sea := mixColour(rl.NewColor(30, 96, 120, 255), rl.NewColor(54, 136, 150, 255), fbm3(rl.Vector3Scale(p, 3), 41))
		edge := .74 + .16*(fbm3(rl.Vector3Scale(p, 5), 42)-.5)
		ice := smoothstep(edge-.04, edge+.04, abs(p.Y))
		c := mixColour(sea, rl.NewColor(226, 238, 244, 255), ice)
		streak := smoothstep(.66, .74, fbm3(rl.Vector3{X: p.X * 2, Y: p.Y * 10, Z: p.Z * 2}, 43))
		return mixColour(c, rl.NewColor(240, 244, 248, 255), streak*.5)
	},
	// g: all ice, cracked across.
	"g": func(p rl.Vector3) rl.Color {
		c := mixColour(rl.NewColor(176, 196, 220, 255), rl.NewColor(222, 232, 242, 255), fbm3(rl.Vector3Scale(p, 3), 51))
		ridge := 1 - abs(2*fbm3(rl.Vector3Scale(p, 5), 52)-1)
		crack := smoothstep(.9, .97, ridge)
		return mixColour(c, rl.NewColor(104, 122, 160, 255), crack*.8)
	},
	// h: the outermost: grey frost with darker patches.
	"h": func(p rl.Vector3) rl.Color {
		patches := smoothstep(.48, .62, fbm3(rl.Vector3Scale(p, 3.5), 61))
		c := mixColour(rl.NewColor(204, 206, 212, 255), rl.NewColor(132, 130, 140, 255), patches*.7)
		return brighten(c, .9+.2*fbm3(rl.Vector3Scale(p, 10), 62))
	},
}

// sunSurface is the sun's face, for a sphere turned with its top pole
// (+Y) to the camera: from the middle of the disc out to its edge it
// darkens and reddens, as a star's does, and it's grained all over with a
// few dark spots.
func sunSurface(p rl.Vector3) rl.Color {
	// How far out from the middle of the disc: 0 at the pole facing the
	// camera, 1 at the edge.
	out := float32(math.Sqrt(float64(max(0, 1-p.Y*p.Y))))
	if p.Y < 0 {
		out = 1
	}
	mu := float32(math.Sqrt(float64(max(0, 1-out*out))))
	face := float32(math.Pow(float64(mu), .6))
	// A red dwarf: orange at the middle, deep red at the edge.
	c := mixColour(rl.NewColor(168, 38, 14, 255), rl.NewColor(255, 168, 84, 255), face)
	grain := .9 + .2*fbm3(rl.Vector3Scale(p, 40), 71)
	spots := smoothstep(.73, .78, fbm3(rl.Vector3Scale(p, 4), 72))
	return brighten(c, grain*(1-.45*spots))
}

// brighten scales c's brightness by k.
func brighten(c rl.Color, k float32) rl.Color {
	s := func(v uint8) uint8 { return uint8(min(255, float32(v)*k)) }
	return rl.NewColor(s(c.R), s(c.G), s(c.B), c.A)
}

// fbm3 is fractal value noise at p: four octaves, 0 to 1.
func fbm3(p rl.Vector3, seed uint32) float32 {
	var sum, amp, total float32 = 0, 1, 0
	for i := range 4 {
		sum += amp * valueNoise3(p, seed*7+uint32(i))
		total += amp
		p, amp = rl.Vector3Scale(p, 2.03), amp*.5
	}
	return sum / total
}

// valueNoise3 is smooth noise in 3D, 0 to 1.
func valueNoise3(p rl.Vector3, seed uint32) float32 {
	x0, y0, z0 := math.Floor(float64(p.X)), math.Floor(float64(p.Y)), math.Floor(float64(p.Z))
	fx, fy, fz := p.X-float32(x0), p.Y-float32(y0), p.Z-float32(z0)
	sx, sy, sz := fx*fx*(3-2*fx), fy*fy*(3-2*fy), fz*fz*(3-2*fz)
	at := func(i, j, k float64) float32 {
		return lattice(int32(i)+int32(k)*7919, int32(j)-int32(k)*104729, seed)
	}
	lerp := func(a, b, t float32) float32 { return a + (b-a)*t }
	bottom := lerp(lerp(at(x0, y0, z0), at(x0+1, y0, z0), sx), lerp(at(x0, y0+1, z0), at(x0+1, y0+1, z0), sx), sy)
	top := lerp(lerp(at(x0, y0, z0+1), at(x0+1, y0, z0+1), sx), lerp(at(x0, y0+1, z0+1), at(x0+1, y0+1, z0+1), sx), sy)
	return lerp(bottom, top, sz)
}
