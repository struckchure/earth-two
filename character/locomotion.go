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

// playerInput turns the keyboard into the player's Intent: WASD or the
// arrows move, Shift runs, Space jumps, E interacts, F punches and Q picks
// up.
func playerInput(q *illusion.Query1Where[Intent, illusion.With[Player]], keys *illusion.Res[input.Keys]) {
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
		in.Run = k.AnyPressed(rl.KeyLeftShift, rl.KeyRightShift)
		if k.JustPressed(rl.KeySpace) {
			in.Jump = true
		}
		if act != Idle {
			in.Act = act
		}
	})
}

// locomote runs in FixedUpdate, feeding each Intent to its character
// controller.
func locomote(q *illusion.Query3[Character, Intent, physics.CharacterController]) {
	q.Each(func(_ ecs.Entity, c *Character, in *Intent, cc *physics.CharacterController) {
		move := in.Move
		move.Y = 0
		if l := rl.Vector3Length(move); l > 1 {
			move = rl.Vector3Scale(move, 1/l)
		}
		speed := c.WalkSpeed
		if in.Run {
			speed = c.RunSpeed
		}
		cc.Walk = rl.Vector3Scale(move, speed)
		if in.Jump && cc.Grounded {
			cc.Velocity.Y = c.JumpSpeed
		}
		in.Jump = false
	})
}

// face turns each body toward where its character is heading. The models
// face +Z.
func face(
	bodies *illusion.Query1Where[transform.Transform, illusion.With[Body]],
	roots *illusion.Query2[Character, physics.CharacterController],
	hier *illusion.Hierarchy,
	t *illusion.Res[illusion.Time],
) {
	dt := t.Get().DeltaSecs()
	bodies.Each(func(e ecs.Entity, tr *transform.Transform) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		c, cc, ok := roots.Get(parent)
		if !ok || cc.Walk.X == 0 && cc.Walk.Z == 0 {
			return
		}
		yaw := float32(math.Atan2(float64(cc.Walk.X), float64(cc.Walk.Z)))
		target := rl.QuaternionFromAxisAngle(transform.Up, yaw)
		tr.Rotation = rl.QuaternionSlerp(tr.Rotation, target, min(1, c.TurnSpeed*dt))
	})
}
