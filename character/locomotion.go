package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// Controls is a resource: whether the keyboard moves the player. A menu
// turns it off while it's open.
type Controls struct {
	Enabled bool
}

// playerInput turns the keyboard into the player's Intent: WASD or the
// arrows move, Shift runs, Space jumps, E interacts, F punches and Q picks
// up. With Controls off, the player stands still.
func playerInput(q *illusion.Query1Where[Intent, illusion.With[Player]], keys *illusion.Res[input.Keys], controls *illusion.Res[Controls]) {
	if !controls.Get().Enabled {
		q.Each(func(_ ecs.Entity, in *Intent) { *in = Intent{} })
		return
	}
	k := keys.Get()
	var dir rl.Vector3
	if k.AnyPressed(rl.KeyA, rl.KeyLeft) {
		dir.X--
	}
	if k.AnyPressed(rl.KeyD, rl.KeyRight) {
		dir.X++
	}
	if k.AnyPressed(rl.KeyW, rl.KeyUp) {
		dir.Z--
	}
	if k.AnyPressed(rl.KeyS, rl.KeyDown) {
		dir.Z++
	}
	if dir != (rl.Vector3{}) {
		dir = rl.Vector3Normalize(dir)
	}
	act := Idle
	switch {
	case k.JustPressed(rl.KeyE):
		act = Interact
	case k.JustPressed(rl.KeyF):
		act = Punch
	case k.JustPressed(rl.KeyQ):
		act = PickUp
	}
	q.Each(func(_ ecs.Entity, in *Intent) {
		in.Move = dir
		in.Slide = in.Slide || k.JustPressed(rl.KeyLeftControl) || k.JustPressed(rl.KeyRightControl)
		in.Roll = in.Roll || k.JustPressed(rl.KeyR)
		in.Run = k.AnyPressed(rl.KeyLeftShift, rl.KeyRightShift)
		if k.JustPressed(rl.KeySpace) {
			in.Jump = true
		}
		if act != Idle {
			in.Act = act
		}
	})
}

const (
	// runTurn is how much slower a body turns at full running speed than
	// at a standstill: the faster it goes, the wider it has to turn.
	runTurn = 0.45
	// slip is the furthest a character's steps stray from where its body
	// faces, and crawl the share of its speed left heading straight back
	// the way it faces: turning about, it slows and walks round in an arc.
	slip  = 50 * math.Pi / 180
	crawl = 0.08
	// brake is how many times quicker than it speeds up a character slows
	// down: feet plant and stop it quicker than they push off.
	brake = 2
	// airSteer is the furthest a character can veer, going or facing, from
	// how it left the ground, and airAccel how fast it can change speed up
	// there (never faster than it took off).
	airSteer = 10 * math.Pi / 180
	airAccel = 2
	// pullUp is how many times quicker than it speeds up a character brakes
	// for an action: from a run to below a walk in about a third of a
	// second.
	pullUp = 2
)

// locomote runs in FixedUpdate, feeding each Intent to its character
// controller. Characters step along where their bodies face (see stride),
// ease their speed up and down (see ease), pull up (and can't jump) for an
// action, and in the air keep to the way they jumped (see airborne).
func locomote(
	q *illusion.Query4[Character, Intent, physics.CharacterController, Traversal],
	bodies *illusion.Query2Where[State, transform.Transform, illusion.With[Body]],
	hier *illusion.Hierarchy,
	t *illusion.Res[illusion.Time],
) {
	dt := t.Get().DeltaSecs()
	q.Each(func(root ecs.Entity, c *Character, in *Intent, cc *physics.CharacterController, traversal *Traversal) {
		if traversal.active() || traversal.impulse {
			in.Jump = false
			return
		}
		move := in.Move
		move.Y = 0
		if l := rl.Vector3Length(move); l > 1 {
			move = rl.Vector3Scale(move, 1/l)
		}
		speed := c.WalkSpeed
		if in.Run || !cc.Grounded {
			// In the air, letting go of Run doesn't slow it: airborne keeps
			// the speed it jumped at.
			speed = c.RunSpeed
		}
		acting := false
		var body *State
		hier.EachChild(root, func(e ecs.Entity) {
			st, tr, ok := bodies.Get(e)
			if !ok {
				return
			}
			body, acting = st, st.Current.OneShot()
			if !cc.Grounded && !st.aloft {
				// Just left the ground: that's the way it's going now.
				st.launch, st.launchYaw = rl.Vector3{X: cc.Walk.X, Z: cc.Walk.Z}, yawOf(tr.Rotation)
			}
			st.aloft = !cc.Grounded
			if l := rl.Vector3Length(move); l > 0 && !st.aloft {
				dir, share := stride(yawOf(tr.Rotation), move)
				move = rl.Vector3Scale(dir, l*share)
			}
		})
		want := rl.Vector3Scale(move, speed)
		if traversal.Land > 0 {
			want = rl.Vector3{} // taking the landing from a wall kick
		}
		switch {
		case traversal.Bounced:
			// Off a wall kick it flies the way it bounced until it lands.
		case body != nil && body.aloft:
			cc.Walk = approach(cc.Walk, airborne(body.launch, want), airAccel*dt)
		case acting:
			cc.Walk = ease(cc.Walk, rl.Vector3{}, pullUp*c.Ease, pullUp*accel(c.Accel), dt)
		default:
			k := float32(1)
			if rl.Vector3Length(want) < rl.Vector3Length(cc.Walk) {
				k = brake
			}
			cc.Walk = ease(cc.Walk, want, k*c.Ease, k*accel(c.Accel), dt)
		}
		if in.Jump && cc.Grounded && !acting {
			cc.Velocity.Y = c.JumpSpeed
		}
		in.Jump = false
	})
}

// airborne is where a character that left the ground going launch (on the
// XZ plane) heads when it wants to go want: it veers at most airSteer from
// launch and never speeds up. Letting go keeps it going; pulling back past
// square on only checks it, to half its speed. Jumping on the spot, it stays
// put.
func airborne(launch, want rl.Vector3) rl.Vector3 {
	top := rl.Vector3Length(launch)
	speed := min(rl.Vector3Length(want), top)
	switch {
	case top < 0.1:
		return rl.Vector3{}
	case speed == 0:
		return launch
	}
	from := float32(math.Atan2(float64(launch.X), float64(launch.Z)))
	off := wrapAngle(float32(math.Atan2(float64(want.X), float64(want.Z))) - from)
	if abs(off) > math.Pi/2 {
		off, speed = 0, top/2
	}
	off = clamp(off, -airSteer, airSteer)
	return rl.Vector3{X: speed * float32(math.Sin(float64(from+off))), Z: speed * float32(math.Cos(float64(from+off)))}
}

// accel is a, or no limit if it's unset.
func accel(a float32) float32 {
	if a <= 0 {
		return math.MaxFloat32
	}
	return a
}

// ease moves v toward want over dt: it closes the share 1 - e^(-rate·dt) of
// the gap (so it slows as it closes in), no faster than most per second,
// and snaps there once within snap. With no rate, it goes at most.
func ease(v, want rl.Vector3, rate, most, dt float32) rl.Vector3 {
	d := rl.Vector3Subtract(want, v)
	l := rl.Vector3Length(d)
	if l <= snap {
		return want
	}
	step := most * dt
	if rate > 0 {
		step = min(step, l*float32(1-math.Exp(float64(-rate*dt))))
	}
	return rl.Vector3Add(v, rl.Vector3Scale(d, min(step, l)/l))
}

// snap is how close (units per second) ease gets before it's there.
const snap = 0.02

// approach moves v toward want by at most step.
func approach(v, want rl.Vector3, step float32) rl.Vector3 {
	d := rl.Vector3Subtract(want, v)
	if l := rl.Vector3Length(d); l > step {
		return rl.Vector3Add(v, rl.Vector3Scale(d, step/l))
	}
	return want
}

// stride is which way a body facing yaw steps when heading along move (on
// the XZ plane), and the share of its speed it can put into it: its steps
// stray at most slip from where it faces, and it slows the further it has
// to turn, sharply past a quarter turn, to crawl of its speed heading
// straight back: turning about, it all but stops and pivots.
func stride(yaw float32, move rl.Vector3) (dir rl.Vector3, share float32) {
	off := wrapAngle(float32(math.Atan2(float64(move.X), float64(move.Z))) - yaw)
	step := yaw + clamp(off, -slip, slip)
	dir = rl.Vector3{X: float32(math.Sin(float64(step))), Z: float32(math.Cos(float64(step)))}
	ahead := float32(1+math.Cos(float64(off))) / 2
	return dir, crawl + (1-crawl)*ahead*ahead
}

// face turns each body toward where its character wants to go, no faster
// than its turnRate, speeding up into the turn and easing out of it; in the
// air, no further than airSteer from how it took off. With nowhere to go,
// or while an action plays, a turn under way winds down. The models face +Z.
func face(
	bodies *illusion.Query2Where[transform.Transform, State, illusion.With[Body]],
	roots *illusion.Query4[Character, Intent, physics.CharacterController, Traversal],
	hier *illusion.Hierarchy,
	t *illusion.Res[illusion.Time],
	controls *illusion.Res[Controls],
) {
	dt := t.Get().DeltaSecs()
	bodies.Each(func(e ecs.Entity, tr *transform.Transform, st *State) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		c, in, cc, traversal, ok := roots.Get(parent)
		if !ok {
			return
		}
		if traversal.active() {
			st.turn = 0
			return
		}
		if traversal.Kick > 0 {
			st.turn = 0
			if controls.Get().Enabled {
				// Off the wall it faces the way it bounced, from the start.
				target := rl.QuaternionFromAxisAngle(transform.Up, st.launchYaw)
				tr.Rotation = rl.QuaternionSlerp(tr.Rotation, target, min(1, dt*28))
			}
			return
		}
		if traversal.Bounced {
			st.turn = 0
			return
		}
		yaw := yawOf(tr.Rotation)
		if in.Move.X == 0 && in.Move.Z == 0 || st.Current.OneShot() {
			if st.turn == 0 {
				return // leave the body be: a menu may be turning it
			}
			st.turn = toward(st.turn, 0, accel(c.TurnAccel)*dt)
			yaw += st.turn * dt
		} else {
			top := turnRate(*c, float32(math.Hypot(float64(cc.Walk.X), float64(cc.Walk.Z))))
			target := float32(math.Atan2(float64(in.Move.X), float64(in.Move.Z)))
			if st.aloft {
				target = st.launchYaw + clamp(wrapAngle(target-st.launchYaw), -airSteer, airSteer)
			}
			yaw, st.turn = turnToward(yaw, st.turn, target, top, accel(c.TurnAccel), dt)
		}
		tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, yaw)
	})
}

// turnRate is the fastest c's body turns going at speed: TurnSpeed at a
// standstill, down to runTurn of it at full running speed.
func turnRate(c Character, speed float32) float32 {
	if c.RunSpeed <= 0 {
		return c.TurnSpeed
	}
	return c.TurnSpeed * (1 - (1-runTurn)*clamp(speed/c.RunSpeed, 0, 1))
}

// turnToward turns from yaw, turning at rate (radians per second), toward
// target over dt: speeding up by at most accel, no faster than top, and
// slowing in time to stop on target. It returns the new yaw and rate.
func turnToward(yaw, rate, target, top, accel, dt float32) (float32, float32) {
	off := wrapAngle(target - yaw)
	if off == 0 && rate == 0 {
		return yaw, 0
	}
	// Straight behind, keep turning the way it already is.
	if math.Abs(float64(off)) > math.Pi-1e-3 && rate != 0 && (off > 0) != (rate > 0) {
		off = -off
	}
	// The fastest it can go and still stop on target.
	want := min(top, float32(math.Sqrt(float64(2*accel*abs(off)))))
	if off < 0 {
		want = -want
	}
	rate = toward(rate, want, accel*dt)
	step := rate * dt
	if (step > 0) == (off > 0) && abs(step) >= abs(off) {
		return target, 0 // there
	}
	return yaw + step, rate
}

// toward moves v toward want by at most step.
func toward(v, want, step float32) float32 {
	if d := want - v; abs(d) > step {
		if d > 0 {
			return v + step
		}
		return v - step
	}
	return want
}

// yawOf is the turn about Up of a rotation that has only that.
func yawOf(q rl.Quaternion) float32 {
	f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, q)
	return float32(math.Atan2(float64(f.X), float64(f.Z)))
}

// wrapAngle brings a into (-π, π].
func wrapAngle(a float32) float32 {
	a = float32(math.Remainder(float64(a), 2*math.Pi))
	if a <= -math.Pi {
		a += 2 * math.Pi
	}
	return a
}

func abs(v float32) float32 { return float32(math.Abs(float64(v))) }
