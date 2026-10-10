package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// In play the camera orbits the player, as in GTA: the mouse turns it about
// them and tilts it up and down, and the keys move them relative to it (see
// character.View). Moving, and leaving the mouse alone for recenterDelay
// seconds, it swings round behind where they face.
const (
	orbitDistance = 4.6
	// orbitPitch is how far it looks down at them, unless turned; it tilts
	// between orbitLow (looking up at the sky) and orbitHigh.
	orbitPitch = 15 * math.Pi / 180
	orbitLow   = -25 * math.Pi / 180
	orbitHigh  = 65 * math.Pi / 180
	// mouseTurn is how far a pixel of mouse movement turns it, in radians.
	mouseTurn = .004
	// recenterDelay is how long after the mouse last moved it starts to
	// follow; recenterRate how quickly it then closes the gap, the share
	// 1 - e^-recenterRate a second.
	recenterDelay = 1.5
	recenterRate  = 2
	// playLag is how quickly the camera keeps up in play; higher is tighter.
	playLag = 12
	// The camera's spring arm, as Godot's SpringArm3D: a sphere armRadius
	// across swept from what it looks at out to where it would be stops
	// armMargin short of the first thing in the way, and the camera there.
	armRadius = .2
	armMargin = .05
	// cameraClearance is the least the camera is above the terrain under
	// it.
	cameraClearance = 1.2
)

// clipNear and clipFar are how near and far the camera sees. raylib's
// near default, 0.01 m, leaves the depth buffer too coarse for the pieces'
// painted-on details at a distance; this is twenty times finer. The spring
// arm keeps the camera further than this from anything. Far reaches past
// the horizon, kilometres off, and the sky beyond it.
const clipNear, clipFar = 0.2, 20000

// orbitLook is the point above the player's capsule centre the camera looks
// at: about their shoulders.
var orbitLook = rl.Vector3{Y: .7}

// orbit is a resource: the camera's place round the player in play. Yaw is
// the way from the player to it about Up (0 behind a player facing -Z),
// and still how long since the mouse last turned it.
type orbit struct {
	yaw, pitch, still float32
	// distance is how far it follows, if not orbitDistance (driving), and
	// exclude what its spring arm passes through, if not the player (the
	// vehicle they're driving): see steerDriving.
	distance float32
	exclude  ecs.Entity
}

func newOrbit() *orbit { return &orbit{pitch: orbitPitch} }

// shot is where the camera is in play.
func (o *orbit) shot() shot {
	cp := float32(math.Cos(float64(o.pitch)))
	dir := rl.Vector3{
		X: float32(math.Sin(float64(o.yaw))) * cp,
		Y: float32(math.Sin(float64(o.pitch))),
		Z: float32(math.Cos(float64(o.yaw))) * cp,
	}
	d := float32(orbitDistance)
	if o.distance > 0 {
		d = o.distance
	}
	return shot{offset: rl.Vector3Add(orbitLook, rl.Vector3Scale(dir, d)), look: orbitLook}
}

// forward is the way the camera faces on the XZ plane.
func (o *orbit) forward() rl.Vector3 {
	return rl.Vector3{X: -float32(math.Sin(float64(o.yaw))), Z: -float32(math.Cos(float64(o.yaw)))}
}

// turn turns the orbit by a mouse movement of delta pixels.
func (o *orbit) turn(delta rl.Vector2) {
	if delta.X == 0 && delta.Y == 0 {
		return
	}
	o.yaw = wrapYaw(o.yaw - delta.X*mouseTurn)
	o.pitch = min(orbitHigh, max(orbitLow, o.pitch+delta.Y*mouseTurn))
	o.still = 0
}

// recenter swings the orbit, over dt seconds, round behind a body facing
// the yaw facing, and back to its usual tilt: if it's moving, the mouse has
// been left alone long enough, and it isn't heading back at the camera
// (following it round then would turn it round and round).
func (o *orbit) recenter(facing float32, moving bool, dt float32) {
	o.still += dt
	if !moving || o.still < recenterDelay {
		return
	}
	behind := wrapYaw(facing + math.Pi)
	if math.Abs(float64(wrapYaw(behind-o.yaw))) > 100*math.Pi/180 {
		return
	}
	k := float32(1 - math.Exp(-recenterRate*float64(dt)))
	o.yaw = wrapYaw(o.yaw + wrapYaw(behind-o.yaw)*k)
	o.pitch += (orbitPitch - o.pitch) * k
}

// wrap brings a into (-π, π].
func wrapYaw(a float32) float32 {
	a = float32(math.Remainder(float64(a), 2*math.Pi))
	if a <= -math.Pi {
		a += 2 * math.Pi
	}
	return a
}

// springArm is where a camera that would be at eye, looking at pivot, can
// be: pulled in to just short of anything in the way between them,
// ignoring exclude (the player).
func springArm(p *physics.Physics, pivot, eye rl.Vector3, exclude ecs.Entity) rl.Vector3 {
	arm := rl.Vector3Subtract(eye, pivot)
	l := rl.Vector3Length(arm)
	if l < 1e-3 {
		return eye
	}
	hit, ok := p.SweepCapsuleExcluding(pivot, arm, armRadius, 2*armRadius, exclude)
	if !ok {
		return eye
	}
	return rl.Vector3Add(pivot, rl.Vector3Scale(arm, max(0, hit.Distance-armMargin)/l))
}

// clearAbove is look, the point above the player's capsule centre at the
// camera looks at, brought down out of anything low it's in: sliding under
// a duct, say, it would be inside it, and the arm would have nowhere to go.
func clearAbove(p *physics.Physics, centre, look rl.Vector3, exclude ecs.Entity) rl.Vector3 {
	return springArm(p, centre, look, exclude)
}

// clock is the frame and fixed-step clocks, for follow (one parameter, to
// stay within Fn8).
type clock struct {
	time  illusion.Res[illusion.Time]
	fixed illusion.Res[illusion.FixedTime]
}

func (c *clock) InitParam(w *ecs.World) {
	c.time.InitParam(w)
	c.fixed.InitParam(w)
}

// pointer is the mouse, for steerCamera (one parameter, to stay within
// Fn8).
type pointer struct {
	mouse   illusion.Res[input.Mouse]
	buttons illusion.Res[input.MouseButtons]
}

func (p *pointer) InitParam(w *ecs.World) {
	p.mouse.InitParam(w)
	p.buttons.InitParam(w)
}

// steerCamera turns the orbit in play by the mouse, follows the player
// round, and points the player's keys the way it faces. It holds the cursor
// while playing; dragging remains a fallback when capture is unavailable.
func steerCamera(
	o *illusion.Res[orbit],
	m *illusion.Res[menu],
	ptr *pointer,
	view *illusion.Res[character.View],
	players *illusion.Query1Where[character.Intent, illusion.And[illusion.With[character.Player], illusion.Without[character.Seated]]],
	bodies *illusion.Query1Where[transform.Transform, illusion.With[character.Body]],
	hier *illusion.Hierarchy,
	t *illusion.Res[illusion.Time],
) {
	or := o.Get()
	playing := m.Get().screen() == playing
	locked := holdCursor(playing)
	if playing && (locked || ptr.buttons.Get().AnyPressed(rl.MouseButtonLeft, rl.MouseButtonRight)) {
		or.turn(cursorDelta(ptr.mouse.Get().Delta))
	}
	moving, facing, found := false, float32(0), false
	players.Each(func(root ecs.Entity, in *character.Intent) {
		moving = in.Move.X != 0 || in.Move.Z != 0
		bodies.Each(func(e ecs.Entity, tr *transform.Transform) {
			if parent, ok := hier.Parent(e); ok && parent == root {
				f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, tr.Rotation)
				facing, found = float32(math.Atan2(float64(f.X), float64(f.Z))), true
			}
		})
	})
	if playing && found {
		or.recenter(facing, moving, t.Get().DeltaSecs())
	}
	view.Get().Forward = or.forward()
}
