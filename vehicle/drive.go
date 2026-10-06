package vehicle

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

const (
	exitSpeed  = 4.0  // m/s: any faster, and it won't let you out
	reverseCap = 9.0  // m/s backwards
	righting   = 1.0  // seconds R is held to right a vehicle
	lost       = -40. // m: below this it's fallen out of the world
	noteFor    = 2.0  // seconds a note stays up
)

// seatAnim is how seat has its sitter sit.
func seatAnim(seat SeatSpec, h Handling) character.Anim {
	switch seat.Pose {
	case "ride":
		return character.Ride
	case "drive", "inside":
		return character.Drive
	}
	if h.Seat != character.Idle {
		return h.Seat
	}
	return character.Drive
}

// reach is points (grips or pegs) in the seat's own frame.
func (s SeatSpec) reach(points [][3]float32) []rl.Vector3 {
	var out []rl.Vector3
	for _, p := range points {
		out = append(out, rl.Vector3Subtract(vec(p), vec(s.At)))
	}
	return out
}

// seatPose is where seat's sitter goes on a vehicle posed at pose.
func seatPose(pose transform.Transform, seat SeatSpec) transform.Transform {
	return transform.Transform{
		Translation: rl.Vector3Add(pose.Translation, rl.Vector3RotateByQuaternion(vec(seat.At), pose.Rotation)),
		Rotation:    pose.Rotation,
		Scale:       rl.Vector3One(),
	}
}

func yawOf(q rl.Quaternion) float32 {
	f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, q)
	return float32(math.Atan2(float64(f.X), float64(f.Z)))
}

// steer drives the player's vehicle from the keys: W and S throttle and
// brake (and reverse, from a stop), A and D steer, Space holds the hand
// brake, E gets out once it's slow enough, and R rights it when it's over.
func steer(
	cars *illusion.Query5[Drivable, physics.VehicleInput, physics.VehicleState, transform.Transform, physics.Velocity],
	drivers *illusion.Query1[character.Seated],
	p *physics.Physics,
	keys *illusion.Res[input.Keys],
	controls *illusion.Res[character.Controls],
	t *illusion.Res[illusion.Time],
	prompt *illusion.Res[Prompt],
	cmd *illusion.Commands,
) {
	k, dt, pr := keys.Get(), t.Get().DeltaSecs(), prompt.Get()
	pr.Key, pr.Text = "", ""
	cars.Each(func(e ecs.Entity, d *Drivable, in *physics.VehicleInput, st *physics.VehicleState, tr *transform.Transform, vel *physics.Velocity) {
		if d.Driver.IsZero() {
			return
		}
		if !drivers.Contains(d.Driver) {
			d.Driver = ecs.Entity{} // gone: despawned, or stood up by something else
			*in = physics.VehicleInput{HandBrake: 1}
			return
		}
		if !controls.Get().Enabled {
			return
		}
		h := d.Handling
		speed := st.Speed

		var throttle, brake float32
		if k.AnyPressed(rl.KeyW, rl.KeyUp) {
			throttle = 1
		}
		if k.AnyPressed(rl.KeyS, rl.KeyDown) {
			throttle = -1
		}
		switch {
		case throttle > 0 && speed < -1.5, throttle < 0 && speed > 1.5:
			// Against the way it's going: brake first, then go.
			throttle, brake = 0, 1
		case throttle > 0 && h.Top > 0 && speed > h.Top, throttle < 0 && speed < -reverseCap:
			throttle = 0
		}
		var want float32
		if k.AnyPressed(rl.KeyD, rl.KeyRight) {
			want++
		}
		if k.AnyPressed(rl.KeyA, rl.KeyLeft) {
			want--
		}
		if h.SteerFade > 0 {
			want /= 1 + float32(math.Abs(float64(speed)))/h.SteerFade
		}
		rate := float32(2.5)
		if want == 0 || want*d.steer < 0 {
			rate = 4 // back to the middle more quickly
		}
		d.steer += max(-rate*dt, min(rate*dt, want-d.steer))
		in.Forward, in.Right, in.Brake, in.HandBrake = throttle, d.steer, brake, 0
		if k.Pressed(rl.KeySpace) {
			in.HandBrake = 1
		}

		// Over on its side or roof: hold R to right it.
		up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, tr.Rotation)
		if up.Y < 0.35 {
			d.upset += dt
		} else {
			d.upset, d.righting = 0, 0
		}
		if d.upset > 1 {
			pr.Key, pr.Text = "R", "Hold to right the "+d.Name
			if k.Pressed(rl.KeyR) {
				d.righting += dt
			} else {
				d.righting = 0
			}
			if d.righting >= righting {
				tr.Translation.Y += 1.5
				tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, yawOf(tr.Rotation))
				*vel = physics.Velocity{}
				d.upset, d.righting = 0, 0
			}
		}
		if tr.Translation.Y < lost {
			tr.Translation, tr.Rotation = rl.Vector3Add(d.Home.Translation, rl.Vector3{Y: 1}), d.Home.Rotation
			*vel = physics.Velocity{}
		}

		if !k.JustPressed(rl.KeyE) {
			return
		}
		if math.Abs(float64(speed)) > exitSpeed {
			pr.note("Slow down to get out")
			return
		}
		for _, exit := range d.Spec.Seats[0].Exits {
			spot := rl.Vector3Add(tr.Translation, rl.Vector3RotateByQuaternion(vec(exit), tr.Rotation))
			hit, ok := p.CastRayExcluding(rl.Vector3Add(spot, rl.Vector3{Y: 1.5}), rl.Vector3{Y: -1}, 4, e)
			if !ok || hit.Normal.Y < 0.6 {
				continue
			}
			feet := hit.Point
			if p.OverlapCapsuleExcluding(rl.Vector3Add(feet, rl.Vector3{Y: 0.92}), 0.3, 1.8, e) {
				continue
			}
			character.Stand(cmd, d.Driver, feet, yawOf(tr.Rotation))
			d.Driver, d.still, d.steer = ecs.Entity{}, 0, 0
			*in = physics.VehicleInput{Brake: 1, HandBrake: 1}
			return
		}
		pr.note("No room to get out here")
	})
	if pr.noteLeft > 0 {
		pr.noteLeft -= dt
	}
}

// offer lets the player into a vehicle: standing near a free seat, E gets
// in (instead of the interact it would be on foot).
func offer(
	players *illusion.Query4Where[character.Intent, physics.CharacterController, character.Traversal, transform.Transform, illusion.With[character.Player]],
	cars *illusion.Query3[Drivable, physics.RigidBody, transform.Transform],
	keys *illusion.Res[input.Keys],
	controls *illusion.Res[character.Controls],
	prompt *illusion.Res[Prompt],
	driving *illusion.Res[Driving],
	physicsSettings *illusion.Res[physics.Settings],
	cmd *illusion.Commands,
) {
	player, in, cc, traversal, at, ok := players.Single()
	if !ok || !cc.Grounded || traversal.Mode != character.Idle || !controls.Get().Enabled || physicsSettings.Get().Paused {
		return
	}
	feet := rl.Vector3Subtract(at.Translation, rl.Vector3{Y: cc.Height / 2})
	var best ecs.Entity
	bestDist := float32(Reach)
	cars.Each(func(e ecs.Entity, d *Drivable, _ *physics.RigidBody, tr *transform.Transform) {
		if !d.Driver.IsZero() {
			return
		}
		seat := seatPose(*tr, d.Spec.Seats[0]).Translation
		dx, dz := seat.X-feet.X, seat.Z-feet.Z
		if dy := seat.Y - feet.Y; dy < -1 || dy > 2 {
			return
		}
		if dist := float32(math.Hypot(float64(dx), float64(dz))); dist < bestDist {
			best, bestDist = e, dist
		}
	})
	if best.IsZero() {
		return
	}
	d, rb, tr, _ := cars.Get(best)
	pr := prompt.Get()
	pr.Key, pr.Text = "E", "Drive the "+d.Name
	if !keys.Get().JustPressed(rl.KeyE) {
		return
	}
	in.Act = character.Idle // getting in, not interacting
	seat := d.Spec.Seats[0]
	character.Sit(cmd, player, character.Seated{Seat: best, Pose: seatPose(*tr, seat), Anim: seatAnim(seat, d.Handling),
		Hidden: seat.Pose == "inside", Hands: seat.reach(seat.Grips), Feet: seat.reach(seat.Pegs)})
	d.Driver, d.still, d.upset, d.righting = player, 0, 0, 0
	*rb = physics.Dynamic
	driving.Get().Vehicle = best
}

// present draws each vehicle between physics steps, its wheels where they
// were simulated, and carries its driver along in the seat.
func present(
	cars *illusion.Query5[Drivable, transform.Transform, physics.RigidBody, physics.Interpolated, physics.VehicleState],
	shells *illusion.Query1Where[transform.Transform, illusion.With[Shell]],
	wheels *illusion.Query2[WheelOf, transform.Transform],
	seated *illusion.Query1[character.Seated],
	hier *illusion.Hierarchy,
	fixed *illusion.Res[illusion.FixedTime],
	driving *illusion.Res[Driving],
) {
	alpha := fixed.Get().Overstep()
	dr := driving.Get()
	dr.Vehicle = ecs.Entity{}
	cars.Each(func(e ecs.Entity, d *Drivable, root *transform.Transform, rb *physics.RigidBody, interp *physics.Interpolated, st *physics.VehicleState) {
		pose := *root
		if *rb == physics.Dynamic {
			pose = interp.At(alpha)
		}
		inverse := rl.QuaternionInvert(root.Rotation)
		hier.EachChild(e, func(child ecs.Entity) {
			shell, ok := shells.Get(child)
			if !ok {
				return
			}
			shell.Translation = rl.Vector3RotateByQuaternion(rl.Vector3Subtract(pose.Translation, root.Translation), inverse)
			shell.Rotation = rl.QuaternionMultiply(inverse, pose.Rotation)
			if *rb != physics.Dynamic || len(st.Wheels) == 0 {
				return
			}
			hier.EachChild(child, func(wheel ecs.Entity) {
				if w, tr, ok := wheels.Get(wheel); ok && w.Index < len(st.Wheels) {
					*tr = st.Wheels[w.Index].Transform
				}
			})
		})
		if d.Driver.IsZero() {
			return
		}
		if s, ok := seated.Get(d.Driver); ok && s.Seat == e {
			s.Pose = seatPose(pose, d.Spec.Seats[0])
		}
		*dr = Driving{Vehicle: e, Name: d.Name, Pose: pose, Camera: d.Spec.chaseDistance(), Speed: st.Speed, Gear: st.Gear}
	})
}

// settle parks a vehicle nobody's driving: it holds the brakes, and once
// it's come to rest (or left the ground that's simulated), it's made Static.
func settle(
	cars *illusion.Query5[Drivable, physics.RigidBody, physics.VehicleInput, physics.VehicleState, transform.Transform],
	ground *illusion.Res[Ground],
	settings *illusion.Res[physics.Settings],
	t *illusion.Res[illusion.Time],
) {
	if settings.Get().Paused {
		return
	}
	dt := t.Get().DeltaSecs()
	covered := func(x, z float32) bool { return true }
	if g, ok := ground.TryGet(); ok && g.Covered != nil {
		covered = g.Covered
	}
	cars.Each(func(_ ecs.Entity, d *Drivable, rb *physics.RigidBody, in *physics.VehicleInput, st *physics.VehicleState, tr *transform.Transform) {
		if !d.Driver.IsZero() || *rb != physics.Dynamic {
			return
		}
		*in = physics.VehicleInput{Brake: 1, HandBrake: 1}
		if math.Abs(float64(st.Speed)) < 0.3 {
			d.still += dt
		} else {
			d.still = 0
		}
		if d.still > 1.5 || !covered(tr.Translation.X, tr.Translation.Z) {
			*rb = physics.Static
			d.still = 0
		}
	})
}

// note shows text for a couple of seconds.
func (p *Prompt) note(text string) { p.Note, p.noteLeft = text, noteFor }
