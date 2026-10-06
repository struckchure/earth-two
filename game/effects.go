package game

import (
	"math"
	"math/rand/v2"
	"runtime"
	"slices"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
)

// The effects: dust kicked up by feet, landings, rolls and slides and
// wheels, and sparks off metal hit hard. Each is a particle, a soft disc
// facing the camera, drawn after the world (so behind what's in front of
// it) far to near, so they blend over each other. They're all one mesh,
// rebuilt each frame: the browser's raylib has no billboards or blend
// modes to draw them one by one, but it can update a mesh.

// maxParticles is the most there can be at once; past it, new ones take
// the place of the oldest.
const maxParticles = 2048

// Dust closer to the camera than nearFade isn't drawn, and is drawn in
// full only from nearClear on.
const (
	nearFade  = 1.5
	nearClear = 6
)

// particle is one mote of dust or spark.
type particle struct {
	pos, vel  rl.Vector3
	age, life float32
	// floor is the ground it rose from: it's drawn no lower than it, so it
	// doesn't cut into the ground in a straight line.
	floor        float32
	size0, size1 float32 // its radius, starting and ending
	colour       rl.Color
	// drag is how quickly it slows (the share 1 - e^-drag a second),
	// fall how fast it accelerates down (m/s², up if less than 0), and
	// blown how much the wind carries it.
	drag, fall, blown float32
}

// effects is a resource: the particles, and the mesh they're drawn as.
type effects struct {
	ps   []particle
	wind rl.Vector3 // the wind the dust drifts on, m/s

	// mesh is its own allocation: cgo checks all of what a pointer passed
	// to raylib points into, and the rest of effects (the particles) isn't
	// pinned.
	mesh   *rl.Mesh
	mat    rl.Material
	ready  bool
	failed bool
	pins   runtime.Pinner
	verts  []float32
	cols   []uint8
	drawn  int // how many quads the mesh had last frame
	order  []drawOrder
}

// drawOrder is a particle to draw, and how far it is ahead of the camera.
type drawOrder struct {
	i     int
	depth float32
}

func newEffects() *effects { return &effects{ps: make([]particle, 0, maxParticles)} }

// add adds p, in place of the oldest if there are already maxParticles.
func (fx *effects) add(p particle) {
	if p.floor == 0 {
		p.floor = p.pos.Y - .05
	}
	if len(fx.ps) < maxParticles {
		fx.ps = append(fx.ps, p)
		return
	}
	oldest := 0
	for i := range fx.ps {
		if fx.ps[i].age/fx.ps[i].life > fx.ps[oldest].age/fx.ps[oldest].life {
			oldest = i
		}
	}
	fx.ps[oldest] = p
}

// The dust's colour in white light: the Red's soil, paler as it's thrown
// up into the air.
var dustColour = rl.NewColor(184, 128, 98, 255)

// lit is c in light: the light's colour over it, partly, as dust in the
// air is lit from all round (it has to read a little paler than the sand
// it's raised from, not glow).
func lit(c, light rl.Color, alpha float32) rl.Color {
	f := func(a, b uint8) uint8 { return uint8(min(255, float32(a)*(.55+.45*float32(b)/255))) }
	return rl.NewColor(f(c.R, light.R), f(c.G, light.G), f(c.B, light.B), uint8(255*clamp01(alpha)))
}

// jitter is a random vector, each part within ±k.
func jitter(k float32) rl.Vector3 {
	r := func() float32 { return (2*rand.Float32() - 1) * k }
	return rl.Vector3{X: r(), Y: r(), Z: r()}
}

// puff is a footstep's dust, at a foot moving at vel (speed on the ground).
func (fx *effects) puff(at, vel rl.Vector3, speed float32, light rl.Color) {
	n := 2 + int(speed/2)
	for range n {
		v := rl.Vector3Add(rl.Vector3Scale(vel, .15), jitter(.35))
		v.Y = .2 + .4*rand.Float32()
		fx.add(particle{
			pos: rl.Vector3Add(at, jitter(.08)), vel: v,
			life: .9 + .6*rand.Float32(), size0: .1, size1: .45 + .25*clamp01(speed/4),
			colour: lit(dustColour, light, .4), drag: 2.5, fall: -.05, blown: .6,
		})
	}
}

// ring is the dust thrown out round something landing hard at at: n motes,
// as far and as big as strength.
func (fx *effects) ring(at rl.Vector3, n int, strength float32, light rl.Color) {
	for i := range n {
		a := 2 * math.Pi * (float64(i) + rand.Float64()) / float64(n)
		out := rl.Vector3{X: float32(math.Cos(a)), Z: float32(math.Sin(a))}
		v := rl.Vector3Scale(out, (1+rand.Float32())*1.4*strength)
		v.Y = .3 + .5*rand.Float32()*strength
		fx.add(particle{
			pos: rl.Vector3Add(at, rl.Vector3{Y: .05}), vel: v,
			life: 1.1 + .8*rand.Float32(), size0: .15, size1: .6 + .5*strength,
			colour: lit(dustColour, light, .45), drag: 2.2, fall: -.05, blown: .7,
		})
	}
}

// trail is the dust a slide or a roll leaves, over dt.
func (fx *effects) trail(at, vel rl.Vector3, dt float32, light rl.Color) {
	n := int(28*dt + rand.Float32())
	for range n {
		v := rl.Vector3Add(rl.Vector3Scale(vel, -.1), jitter(.4))
		v.Y = .2 + .5*rand.Float32()
		fx.add(particle{
			pos: rl.Vector3Add(at, jitter(.15)), vel: v,
			life: 1 + .6*rand.Float32(), size0: .15, size1: .7,
			colour: lit(dustColour, light, .4), drag: 2, fall: -.05, blown: .8,
		})
	}
}

// wheel is one puff of the dust a wheel throws up, going vel, as hard as k
// (0 to 1).
func (fx *effects) wheel(at, vel rl.Vector3, k float32, light rl.Color) {
	v := rl.Vector3Add(rl.Vector3Scale(vel, -.08), jitter(.8))
	v.Y = .4 + 1.2*rand.Float32()*k
	fx.add(particle{
		pos: rl.Vector3Add(at, jitter(.2)), vel: v,
		life: 1.4 + 1.2*rand.Float32()*k, size0: .25, size1: .8 + 1.8*k,
		colour: lit(dustColour, light, .18+.27*k), drag: 1.6, fall: -.08, blown: 1,
	})
}

// sparks are n sparks off metal hit at at, thrown back out along normal.
func (fx *effects) sparks(at, normal rl.Vector3, n int) {
	out := rl.Vector3Negate(rl.Vector3Normalize(normal))
	for range n {
		v := rl.Vector3Add(rl.Vector3Scale(out, 2+4*rand.Float32()), jitter(3))
		v.Y += 1.5 * rand.Float32()
		fx.add(particle{
			pos: at, vel: v,
			life: .25 + .35*rand.Float32(), size0: .05, size1: .015,
			colour: rl.NewColor(255, 214, 140, 255), drag: .6, fall: 9.8,
		})
	}
}

// Dust drifts on the wind: a breath of it on a clear day, the storm's in
// a storm.
const (
	calmWind  = .4
	stormWind = 6
)

// moveEffects ages and moves the particles, and drops those past their
// life.
func moveEffects(fx *illusion.Res[effects], w *illusion.Res[weather], clk *illusion.Res[illusion.Time]) {
	e, dt := fx.Get(), clk.Get().DeltaSecs()
	e.wind = rl.Vector3Scale(rl.Vector3Normalize(dustWind), calmWind+(stormWind-calmWind)*w.Get().storm)
	e.step(dt)
}

func (fx *effects) step(dt float32) {
	kept := fx.ps[:0]
	for _, p := range fx.ps {
		p.age += dt
		if p.age >= p.life {
			continue
		}
		slow := float32(math.Exp(-float64(p.drag * dt)))
		p.vel = rl.Vector3Scale(p.vel, slow)
		// The wind carries it toward its own speed.
		p.vel = rl.Vector3Add(p.vel, rl.Vector3Scale(rl.Vector3Subtract(fx.wind, p.vel), p.blown*(1-slow)))
		p.vel.Y -= p.fall * dt
		p.pos = rl.Vector3Add(p.pos, rl.Vector3Scale(p.vel, dt))
		kept = append(kept, p)
	}
	fx.ps = kept
}

// The particles' shader: a soft disc in the vertex colour, its edge
// fading out.
const (
	effectsVertex = `
in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec4 vertexColor;
uniform mat4 mvp;
out vec2 uv;
out vec4 tint;
void main() {
    uv = vertexTexCoord;
    tint = vertexColor;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
`
	effectsFragment = `
in vec2 uv;
in vec4 tint;
out vec4 finalColor;
void main() {
    float r = length(uv * 2.0 - 1.0);
    // Soft all the way in: dust thins out from its middle, it has no rim.
    float a = tint.a * pow(1.0 - smoothstep(0.0, 1.0, r), 1.6);
    if (a < 0.01) {
        discard;
    }
    finalColor = vec4(tint.rgb, a);
}
`
)

// build makes the mesh and the material, the first time there's something
// to draw.
func (fx *effects) build() {
	n := maxParticles
	fx.verts = make([]float32, 4*3*n)
	uvs := make([]float32, 4*2*n)
	fx.cols = make([]uint8, 4*4*n)
	idx := make([]uint16, 6*n)
	for i := range n {
		copy(uvs[8*i:], []float32{0, 1, 1, 1, 1, 0, 0, 0})
		b := uint16(4 * i)
		copy(idx[6*i:], []uint16{b, b + 1, b + 2, b, b + 2, b + 3})
	}
	fx.mesh = &rl.Mesh{
		VertexCount: int32(4 * n), TriangleCount: int32(2 * n),
		Vertices: &fx.verts[0], Texcoords: &uvs[0], Colors: &fx.cols[0], Indices: &idx[0],
	}
	// raylib keeps pointers to these Go arrays in the mesh.
	fx.pins.Pin(&fx.verts[0])
	fx.pins.Pin(&uvs[0])
	fx.pins.Pin(&fx.cols[0])
	fx.pins.Pin(&idx[0])
	rl.UploadMesh(fx.mesh, true)
	shader := rl.LoadShaderFromMemory(effectsVertexHeader+effectsVertex, effectsFragmentHeader+effectsFragment)
	if shader.ID == 0 {
		fx.failed = true
		return
	}
	fx.mat = rl.LoadMaterialDefault()
	fx.mat.Shader = shader
	fx.ready = true
}

// drawEffects draws the particles, far to near, facing the camera.
func drawEffects(fx *illusion.Res[effects], view *illusion.Res[render.View3D]) {
	e, v := fx.Get(), view.Get()
	if !v.Active || e.failed || (len(e.ps) == 0 && e.drawn == 0) {
		return
	}
	if !e.ready {
		e.build()
		if !e.ready {
			return
		}
	}
	cam := v.Camera
	ahead := rl.Vector3Normalize(rl.Vector3Subtract(cam.Target, cam.Position))
	right := rl.Vector3Normalize(rl.Vector3CrossProduct(ahead, cam.Up))
	up := rl.Vector3CrossProduct(right, ahead)
	n := e.quads(cam.Position, ahead, right, up)
	used := max(n, e.drawn)
	e.drawn = n
	if used == 0 {
		return
	}
	rl.UpdateMeshBuffer(*e.mesh, 0, floatBytes(e.verts[:12*used]), 0)
	rl.UpdateMeshBuffer(*e.mesh, 3, e.cols[:16*used], 0)
	rl.DrawMesh(*e.mesh, e.mat, rl.MatrixIdentity())
}

// quads writes the particles into the mesh's arrays, far to near, and
// returns how many; the quads past them, left from a frame with more, are
// collapsed to nothing.
func (fx *effects) quads(eye, ahead, right, up rl.Vector3) int {
	fx.order = fx.order[:0]
	for i, p := range fx.ps {
		d := rl.Vector3DotProduct(rl.Vector3Subtract(p.pos, eye), ahead)
		if d < .2 {
			continue // behind the camera, or in its face
		}
		fx.order = append(fx.order, drawOrder{i, d})
	}
	slices.SortFunc(fx.order, func(a, b drawOrder) int {
		switch {
		case a.depth > b.depth:
			return -1
		case a.depth < b.depth:
			return 1
		}
		return 0
	})
	for q, o := range fx.order {
		p := fx.ps[o.i]
		t := p.age / p.life
		s := p.size0 + (p.size1-p.size0)*t
		// Held up off the ground by its size: a mote down in it would be
		// cut off flat.
		p.pos.Y = max(p.pos.Y, p.floor+s*abs(up.Y))
		r, u := rl.Vector3Scale(right, s), rl.Vector3Scale(up, s)
		corners := [4]rl.Vector3{
			rl.Vector3Subtract(rl.Vector3Subtract(p.pos, r), u),
			rl.Vector3Subtract(rl.Vector3Add(p.pos, r), u),
			rl.Vector3Add(rl.Vector3Add(p.pos, r), u),
			rl.Vector3Add(rl.Vector3Subtract(p.pos, r), u),
		}
		for c, at := range corners {
			copy(fx.verts[12*q+3*c:], []float32{at.X, at.Y, at.Z})
		}
		// In quickly, out slowly.
		fade := smoothstep(0, .12, t) * (1 - smoothstep(.45, 1, t))
		// Thinned out close to the camera, so following a vehicle through
		// its own dust doesn't fill the view with it.
		fade *= smoothstep(nearFade, nearClear, o.depth)
		col := p.colour
		a := uint8(float32(col.A) * fade)
		for c := range 4 {
			copy(fx.cols[16*q+4*c:], []uint8{col.R, col.G, col.B, a})
		}
	}
	n := len(fx.order)
	for q := n; q < fx.drawn && q < maxParticles; q++ {
		clear(fx.verts[12*q : 12*q+12])
	}
	return n
}
