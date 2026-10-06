package vehicle

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// Tyre is a parked vehicle's tyre as something to bump into: a static
// cylinder where the wheel is. A vehicle's own body is only its chassis,
// which clears the ground and so its wheels, for it to ride on them; while
// it's parked, its tyres stop the player walking through them, and other
// vehicles driving into them. They go when it's driven, or the wheels'
// casts for the ground would find them.
type Tyre struct {
	Of ecs.Entity
}

// tyres gives each parked vehicle its tyres, where its wheels came to rest,
// and takes them away while it's driven.
func tyres(
	cars *illusion.Query4[Drivable, physics.RigidBody, transform.Transform, physics.VehicleState],
	existing *illusion.Query1[Tyre],
	cmd *illusion.Commands,
) {
	cars.Each(func(e ecs.Entity, d *Drivable, rb *physics.RigidBody, root *transform.Transform, st *physics.VehicleState) {
		parked := *rb == physics.Static
		switch {
		case parked && !d.tyred:
			axle := rl.QuaternionMultiply(root.Rotation, rl.QuaternionFromAxisAngle(rl.Vector3{Z: 1}, math.Pi/2))
			for i, w := range d.Spec.Wheels {
				at := vec(w.At)
				if i < len(st.Wheels) {
					at = st.Wheels[i].Transform.Translation
				}
				centre := rl.Vector3Add(root.Translation, rl.Vector3RotateByQuaternion(at, root.Rotation))
				cmd.Spawn(
					illusion.C(Tyre{Of: e}),
					illusion.C(transform.FromTranslation(centre).WithRotation(axle)),
					illusion.C(physics.Static),
					illusion.C(physics.Cylinder(w.Radius, w.Width)),
				)
			}
			d.tyred = true
		case !parked && d.tyred:
			existing.Each(func(t ecs.Entity, tyre *Tyre) {
				if tyre.Of == e {
					cmd.Despawn(t)
				}
			})
			d.tyred = false
		}
	})
}
