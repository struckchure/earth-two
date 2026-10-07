package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// Teleporting (part of the test plugin, testing.go), to test a place
// without the drive there: P on the full map
// takes the player to the point under the pointer, and P in play to the
// destination marked on the map. Driving, the vehicle goes too, with them
// in it. They land on the ground, or if there's no room to stand there (in
// a wall, under a deck), on top of what's there.
const teleportKey = rl.KeyP

// movers are what a teleport moves: the player on foot, the vehicle
// they're driving, and the camera.
type movers struct {
	players illusion.Query3Where[transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Player]]
	cars    illusion.Query2Where[transform.Transform, physics.Velocity, illusion.With[vehicle.Drivable]]
	cameras illusion.Query1Where[transform.Transform, illusion.With[render.Camera3d]]
	driving illusion.Res[vehicle.Driving]
	orbit   illusion.Res[orbit]
}

func (m *movers) InitParam(w *ecs.World) {
	m.players.InitParam(w)
	m.cars.InitParam(w)
	m.cameras.InitParam(w)
	m.driving.InitParam(w)
	m.orbit.InitParam(w)
}

// screenInput is the keyboard and pointer, and the window they're over.
type screenInput struct {
	keys  illusion.Res[input.Keys]
	mouse illusion.Res[input.Mouse]
	win   illusion.Res[window.Window]
	fonts illusion.Res[uiFonts]
}

func (s *screenInput) InitParam(w *ecs.World) {
	s.keys.InitParam(w)
	s.mouse.InitParam(w)
	s.win.InitParam(w)
	s.fonts.InitParam(w)
}

func teleport(in *screenInput, m *illusion.Res[menu], wm *illusion.Res[worldMap], mv *movers, p *physics.Physics, jobs *illusion.Res[contracts]) {
	if !in.keys.Get().JustPressed(teleportKey) {
		return
	}
	wmap, ok := wm.TryGet()
	if !ok {
		return
	}
	var to rl.Vector2
	switch m.Get().screen() {
	case mapping:
		ww, ok := in.win.TryGet()
		if !ok {
			return
		}
		fonts, _ := in.fonts.TryGet()
		pt := newPainter(fonts, ww)
		f := fullMapFrame(pt, wmap, float32(ww.Width), float32(ww.Height))
		at := in.mouse.Get().Position
		if !contains(f.screen, at) {
			return
		}
		to = f.toWorld(at)
		m.Get().back()
	case playing:
		navigation, _ := contractRoute(wmap, jobs)
		if !navigation.marked {
			return
		}
		to = navigation.dest
	default:
		return
	}
	b := wmap.bounds
	if b.Width > 0 {
		to = rl.Vector2{X: min(b.X+b.Width, max(b.X, to.X)), Y: min(b.Y+b.Height, max(b.Y, to.Y))}
	}
	moveTo(mv, p, to)
}

// moveTo takes the player, or the vehicle they're driving, to (X, Z) to,
// and the camera with them.
func moveTo(mv *movers, p *physics.Physics, to rl.Vector2) {
	var from rl.Vector3
	d := mv.driving.Get()
	if d.Active() {
		tr, v, ok := mv.cars.Get(d.Vehicle)
		if !ok {
			return
		}
		from = tr.Translation
		feet := landing(p, to, d.Vehicle)
		// A little above, to settle onto its wheels; facing as it was, upright.
		tr.Translation = rl.Vector3Add(feet, rl.Vector3{Y: .4})
		f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, tr.Rotation)
		tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, float32(math.Atan2(float64(f.X), float64(f.Z))))
		*v = physics.Velocity{}
	} else {
		e, tr, cc, traversal, ok := mv.players.Single()
		if !ok {
			return
		}
		from = tr.Translation
		feet := landing(p, to, e)
		tr.Translation = rl.Vector3Add(feet, rl.Vector3{Y: cc.Height/2 + .05})
		cc.Velocity, cc.Walk = rl.Vector3{}, rl.Vector3{}
		*traversal = character.Traversal{}
	}
	// The camera comes along, rather than sweeping across the map to them.
	mv.cameras.Each(func(_ ecs.Entity, cam *transform.Transform) {
		moved := rl.Vector3Subtract(rl.Vector3{X: to.X, Y: cam.Translation.Y, Z: to.Y}, rl.Vector3{X: from.X, Y: cam.Translation.Y, Z: from.Z})
		cam.Translation = rl.Vector3Add(cam.Translation, moved)
		cam.Translation.Y = max(cam.Translation.Y, groundHeight(cam.Translation.X, cam.Translation.Z)+groundLevel+cameraClearance)
	})
	mv.orbit.Get().still = 0
}

// landing is where to stand at (X, Z) at: on the ground, if a standing
// capsule fits there, or else on top of whatever's there (a deck, a roof).
// The ground's colliders only exist round the player, so the ground is the
// terrain's height; what's built on it always collides.
func landing(p *physics.Physics, at rl.Vector2, exclude ecs.Entity) rl.Vector3 {
	ground := rl.Vector3{X: at.X, Y: walkHeight(at.X, at.Y), Z: at.Y}
	const above = 3000
	hit, ok := p.CastRayExcluding(rl.Vector3{X: at.X, Y: above, Z: at.Y}, rl.Vector3{Y: -1}, 2*above, exclude)
	if !ok || hit.Point.Y <= ground.Y+.2 {
		return ground
	}
	if !p.OverlapCapsuleExcluding(rl.Vector3Add(ground, rl.Vector3{Y: .95}), .3, 1.8, exclude) {
		// Room to stand on the ground, under whatever's above.
		under, blocked := p.CastRayExcluding(rl.Vector3Add(ground, rl.Vector3{Y: .1}), rl.Vector3{Y: 1}, 1.9, exclude)
		if !blocked || under.Distance >= 1.9 {
			return ground
		}
	}
	return hit.Point
}
