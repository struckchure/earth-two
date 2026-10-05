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

// drawDistance is how far away the world's pieces are drawn, from the
// camera to the nearest edge of each: Landfall's are a few thousand pieces,
// and the engine draws every one it isn't told to hide, twice with its
// outline, behind the camera or not.
const drawDistance = 160

// sphere is a model's bounds as a sphere about its middle, in its own
// frame.
type sphere struct {
	center rl.Vector3
	radius float32
}

// culling is cull's memory: each model's bounds, and which pieces it has
// hidden.
type culling struct {
	bounds map[asset.Handle[render.Model]]sphere
	hidden map[ecs.Entity]bool
}

// cull hides the world's pieces the camera can't see, so they're not drawn:
// those behind it, off the sides of the view, or further than drawDistance.
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
		far:    drawDistance,
	}
	v.right = rl.Vector3CrossProduct(v.ahead, v.up)
	s := state.Get()
	if s.bounds == nil {
		s.bounds, s.hidden = map[asset.Handle[render.Model]]sphere{}, map[ecs.Entity]bool{}
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
		hide := !v.sees(center, b.radius)
		if hide != s.hidden[e] {
			s.hidden[e] = hide
			if hide {
				cmd.Entity(e).Insert(illusion.C(render.Hidden{}))
			} else {
				cmd.Entity(e).Remove(ecs.C[render.Hidden]())
			}
		}
	})
}

// view is what a camera sees: from at, looking ahead, with up and right
// across its view; tanV is the tangent of half its vertical field of view,
// aspect its width over its height, and far how far it sees.
type view struct {
	at, ahead, up, right rl.Vector3
	tanV, aspect, far    float32
}

// sees reports whether any of a sphere about center of radius is in view.
func (v view) sees(center rl.Vector3, radius float32) bool {
	d := rl.Vector3Subtract(center, v.at)
	if rl.Vector3Length(d)-radius > v.far {
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
