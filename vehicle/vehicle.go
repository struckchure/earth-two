package vehicle

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Drivable is a vehicle the player can get into. It goes on the root, the
// physics body.
type Drivable struct {
	Name     string // what the HUD calls it: "buggy"
	Spec     *Spec
	Handling Handling
	// Driver is who's in the driving seat; the zero entity for nobody.
	Driver ecs.Entity
	// Home is where it was parked to begin with, for when it's lost off the
	// edge of the world.
	Home transform.Transform
	// Headlamps is the current power state. Automatic lamps follow the
	// driver and the night; lamps explicitly switched on stay on when parked.
	Headlamps  bool
	lightsMode headlampMode

	steer    float32 // the steering as eased toward the keys
	still    float32 // seconds at rest with nobody in it
	upset    float32 // seconds on its side or roof
	righting float32 // seconds R has been held to right it
	tyred    bool    // whether its Tyres are out, parked
}

// Shell marks a vehicle's chassis model, a child of its root.
type Shell struct{}

// WheelOf marks a wheel model, a child of its vehicle's Shell.
type WheelOf struct{ Index int }

// Look is the components that draw a model: render.Model3d, and an outline
// pass, say.
type Look func(model asset.Handle[render.Model]) []illusion.Component

// Spawn parks a drivable vehicle: the piece name, whose model is model and
// spec spec, at at turned by turn. wheel looks up each wheel's piece's
// model. It's Static until someone gets in.
func Spawn(cmd *illusion.Commands, name string, spec *Spec, model asset.Handle[render.Model],
	wheel func(piece string) asset.Handle[render.Model], look Look, at rl.Vector3, turn rl.Quaternion,
) (illusion.EntityCommands, error) {
	h, ok := Handlings[spec.Handling]
	if !ok {
		return illusion.EntityCommands{}, fmt.Errorf("vehicle %s: no handling %q", name, spec.Handling)
	}
	if len(spec.Wheels) == 0 || len(spec.Chassis) == 0 || len(spec.Seats) == 0 {
		return illusion.EntityCommands{}, fmt.Errorf("vehicle %s needs wheels, a chassis and a seat", name)
	}
	home := transform.FromTranslation(at).WithRotation(turn)
	collider := physics.ConvexHull(spec.corners())
	com := vec(spec.CenterOfMass)
	if com == (rl.Vector3{}) {
		// Under the middle of the chassis, as low as the axles: heavy
		// things (batteries, motors, the floor) ride low.
		b := spec.bounds()
		com = rl.Vector3Scale(rl.Vector3Add(b.Min, b.Max), 0.5)
		com.Y = 0
		for _, w := range spec.Wheels {
			com.Y += w.At[1] / float32(len(spec.Wheels))
		}
	}
	collider = collider.WithCenterOfMass(com)

	root := cmd.Spawn(
		illusion.C(Drivable{Name: name, Spec: spec, Handling: h, Home: home}),
		illusion.C(home),
		illusion.C(physics.Static),
		illusion.C(collider),
		illusion.C(physics.Mass(h.Mass)),
		illusion.C(physics.ContinuousCollision{}),
		illusion.C(physics.Material{Friction: 0.4}),
		illusion.C(build(spec, h, com)),
		illusion.C(physics.VehicleInput{HandBrake: 1}),
		illusion.C(physics.VehicleState{}),
		illusion.C(physics.Velocity{}),
		illusion.C(physics.Interpolated{}),
	)
	root.WithChildren(func(c *illusion.ChildBuilder) {
		shell := c.Spawn(append([]illusion.Component{illusion.C(Shell{}), illusion.C(transform.Identity())}, look(model)...)...)
		shell.WithChildren(func(c *illusion.ChildBuilder) {
			spawnHeadlamps(c, spec)
			for i, w := range spec.Wheels {
				c.Spawn(append([]illusion.Component{illusion.C(WheelOf{Index: i}), illusion.C(parkedWheel(w))},
					look(wheel(w.Piece))...)...)
			}
		})
	})
	return root, nil
}

// parkedWheel is a wheel's model where it's modelled, turned about for the
// right side.
func parkedWheel(w WheelSpec) transform.Transform {
	tr := transform.FromTranslation(vec(w.At))
	if !w.Left {
		tr.Rotation = rl.QuaternionFromAxisAngle(transform.Up, math.Pi)
	}
	return tr
}

// Prompt is what the HUD offers the player: Key to do Text ("E", "Drive
// buggy"), or, with no Key, just a note ("Slow down to get out").
type Prompt struct {
	Key, Text string
	// Note is a passing message, shown for a couple of seconds.
	Note     string
	noteLeft float32
}

// Noting is the note to show now, if any.
func (p *Prompt) Noting() string {
	if p.noteLeft > 0 {
		return p.Note
	}
	return ""
}

// Driving is a resource saying what the player is driving, for the camera
// and the HUD.
type Driving struct {
	Vehicle ecs.Entity // the zero entity on foot
	Name    string
	// Pose is the vehicle's drawn pose this frame.
	Pose transform.Transform
	// Camera is how far behind to follow.
	Camera    float32
	Speed     float32 // m/s, forward
	Gear      int
	Headlamps bool
}

// Active reports whether the player is driving.
func (d *Driving) Active() bool { return !d.Vehicle.IsZero() }

// Ground is an optional resource the game inserts: whether there's ground
// to collide with at x, z. A vehicle left where there isn't any is parked
// (made Static) at once, before it can fall through the world.
type Ground struct {
	Covered func(x, z float32) bool
}

// Reach is how close (m) the player must be to a seat to get in.
const Reach = 2.5

// Plugin runs the vehicles. It needs character.Plugin and physics.Plugin.
type Plugin struct{}

func (Plugin) Build(app *illusion.App) {
	app.InsertResource(illusion.R(&Prompt{}), illusion.R(&Driving{}), illusion.R(&LightCycle{}))
	app.InsertResource(illusion.R(&pedestrianPhysics{every: map[ecs.Entity]int{}}))
	app.InitResource(illusion.R(&ImpactRules{CriticalSpeed: 10 / 3.6, LethalSpeed: 30 / 3.6}))
	app.InsertResource(illusion.R(&impactMemory{cars: map[ecs.Entity]impactCar{}, people: map[ecs.Entity]rl.Vector3{}, hits: map[ecs.Entity]vehicleImpact{}}))
	app.AddSystems(illusion.Update, illusion.Chain(
		illusion.Fn8(steer),
		illusion.Fn8(offer),
		illusion.Fn8(present),
		illusion.Fn3(tyres),
	).After(character.Input).Before(character.Act))
	app.AddSystems(illusion.FixedUpdate, illusion.Fn4(settle))
	app.AddSystems(illusion.FixedPostUpdate,
		illusion.Fn8(runOver).Before(physics.Prepare),
		illusion.Fn4(preparePedestrians).After(physics.Prepare).Before(physics.Step),
		illusion.Fn2(restorePedestrianSteps).After(physics.Writeback),
		illusion.Fn5(impactContacts).After(physics.Writeback),
	)
	app.AddSystems(illusion.PostUpdate, illusion.Fn7(updateHeadlamps).Before(transform.Propagate))
	app.AddSystems(illusion.Render, illusion.Fn6(drawHeadlamps).InSet(render.Draw3D))
}
