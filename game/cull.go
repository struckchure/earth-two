package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// drawDistance is how far away the world's small pieces are drawn, from the
// camera to the nearest edge of each: there are thousands, and the engine
// draws every one it isn't told to hide, twice with its outline, behind the
// camera or not. Bigger pieces are drawn further, by sightRange: the dome,
// the Second Light and the drifter are landmarks across the Fringe.
const (
	drawDistance = 160
	sightRange   = 25 // more metres away per square metre of radius
	sightMax     = 12000
)

// sight is how far away a piece of radius r is drawn.
func sight(r float32) float32 { return min(sightMax, drawDistance+sightRange*r*r) }

// sphere is a model's bounds as a sphere about its middle, in its own
// frame.
type sphere struct {
	center rl.Vector3
	radius float32
}

// culling is cull's memory: each model's bounds, and how it's told the
// engine to draw each piece.
type culling struct {
	bounds map[asset.Handle[render.Model]]sphere
	drawn  map[ecs.Entity]drawn
}

// drawn is how a piece is drawn: to the camera (and into the shadow map),
// only into the shadow map, or not at all.
type drawn int

const (
	seen drawn = iota
	shadowOnly
	unseen
)

// shadowReach is how far from the camera pieces out of view are still drawn
// into the shadow map, so they throw their shadows into it: past the
// shadowed square round the view (shadowRange), as the sun is low and the
// shadows long.
const shadowReach = 2 * shadowRange

// cull hides the world's pieces the camera can't see, so they're not drawn:
// those behind it, off the sides of the view, or further than drawDistance.
// Those out of view but near enough to throw a shadow into it are drawn
// only into the shadow map.
// The pieces are the models standing on their own: not posed, not a part
// of anything (characters and what they wear are left alone). It tells the
// engine only when one changes.
func cull(
	cmd *illusion.Commands,
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
	pieces *illusion.Query2Where[render.Model3d, transform.GlobalTransform, illusion.Without[render.AnimationPlayer]],
	models *illusion.Res[asset.Assets[render.Model]],
	win *illusion.Res[window.Window],
	hier *illusion.Hierarchy,
	state *illusion.Local[culling],
	tiles *illusion.Query2[transform.Transform, terrainTile],
) {
	_, eye, cam, ok := cameras.Single()
	if !ok {
		return
	}
	ww := win.Get()
	fovy := cam.Fovy
	if fovy == 0 {
		fovy = 45
	}
	v := view{
		at:     eye.Translation,
		ahead:  eye.Forward(),
		up:     rl.Vector3RotateByQuaternion(transform.Up, eye.Rotation),
		tanV:   float32(math.Tan(float64(fovy) * math.Pi / 360)),
		aspect: float32(ww.Width) / max(float32(ww.Height), 1),
	}
	v.right = rl.Vector3CrossProduct(v.ahead, v.up)
	s := state.Get()
	if s.bounds == nil {
		s.bounds, s.drawn = map[asset.Handle[render.Model]]sphere{}, map[ecs.Entity]drawn{}
	}
	store := models.Get()
	pieces.Each(func(e ecs.Entity, m *render.Model3d, g *transform.GlobalTransform) {
		if _, child := hier.Parent(e); child {
			return
		}
		b, ok := s.bounds[m.Model]
		if !ok {
			model := store.Get(m.Model)
			if model == nil {
				return // not loaded yet
			}
			box := rl.GetModelBoundingBox(model.Model)
			b = sphere{
				center: rl.Vector3Scale(rl.Vector3Add(box.Min, box.Max), .5),
				radius: rl.Vector3Distance(box.Min, box.Max) / 2,
			}
			s.bounds[m.Model] = b
		}
		center := rl.Vector3Transform(b.center, g.Matrix)
		switch {
		case v.sees(center, b.radius, sight(b.radius)):
			s.show(cmd, e, seen)
		case rl.Vector3Distance(center, v.at)-b.radius < shadowReach:
			s.show(cmd, e, shadowOnly)
		default:
			s.show(cmd, e, unseen)
		}
	})
	// The terrain's tiles, out to the horizon, but those off to the sides
	// and behind.
	tiles.Each(func(e ecs.Entity, tr *transform.Transform, _ *terrainTile) {
		d := unseen
		if v.sees(rl.Vector3Add(tr.Translation, rl.Vector3{Y: 40}), tileSize*.75, clipFar) {
			d = seen
		}
		s.show(cmd, e, d)
	})
}

// show tells the engine how to draw e, if that's changed.
func (s *culling) show(cmd *illusion.Commands, e ecs.Entity, d drawn) {
	was := s.drawn[e]
	if d == was {
		return
	}
	s.drawn[e] = d
	switch was {
	case shadowOnly:
		cmd.Entity(e).Remove(ecs.C[render.ShadowOnly]())
	case unseen:
		cmd.Entity(e).Remove(ecs.C[render.Hidden]())
	}
	switch d {
	case shadowOnly:
		cmd.Entity(e).Insert(illusion.C(render.ShadowOnly{}))
	case unseen:
		cmd.Entity(e).Insert(illusion.C(render.Hidden{}))
	}
}

// view is what a camera sees: from at, looking ahead, with up and right
// across its view; tanV is the tangent of half its vertical field of view,
// and aspect its width over its height.
type view struct {
	at, ahead, up, right rl.Vector3
	tanV, aspect         float32
}

// sees reports whether any of a sphere about center of radius is in view,
// no further than far.
func (v view) sees(center rl.Vector3, radius, far float32) bool {
	d := rl.Vector3Subtract(center, v.at)
	if rl.Vector3Length(d)-radius > far {
		return false
	}
	z := rl.Vector3DotProduct(d, v.ahead)
	if z < -radius {
		return false // behind
	}
	// Outside a side of the view: further across than the view is wide at
	// that depth, by more than the sphere reaches (the side planes slope,
	// so a sphere touching one reaches radius/cos across).
	edge := func(across, tan float32) bool {
		return float32(math.Abs(float64(across)))-z*tan > radius*float32(math.Sqrt(float64(1+tan*tan)))
	}
	tanH := v.tanV * v.aspect
	return !edge(rl.Vector3DotProduct(d, v.right), tanH) && !edge(rl.Vector3DotProduct(d, v.up), v.tanV)
}
