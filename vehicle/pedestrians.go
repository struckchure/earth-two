package vehicle

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// Crowd LOD may skip character updates, leaving a stationary kinematic
// capsule between them. Near a moving car that capsule can stop the car
// before the character controller has a chance to respond and move aside.
// Temporarily step nearby pedestrians every tick, then restore their LOD.
type pedestrianPhysics struct {
	every map[ecs.Entity]int
	cars  []movingChassis
}

type movingChassis struct {
	at, velocity rl.Vector3
	radius       float32
}

func preparePedestrians(
	people *illusion.Query2[physics.CharacterController, transform.Transform],
	cars *illusion.Query4[Drivable, physics.RigidBody, transform.Transform, physics.Velocity],
	state *illusion.Res[pedestrianPhysics],
	clock *illusion.Res[illusion.Time],
) {
	s := state.Get()
	s.cars = s.cars[:0]
	cars.Each(func(_ ecs.Entity, car *Drivable, body *physics.RigidBody, tr *transform.Transform, velocity *physics.Velocity) {
		if *body != physics.Dynamic {
			return
		}
		s.cars = append(s.cars, movingChassis{at: tr.Translation, velocity: velocity.Linear, radius: chassisRadius(car.Spec)})
	})
	if len(s.cars) == 0 {
		return
	}
	dt := clock.Get().DeltaSecs()
	people.Each(func(e ecs.Entity, cc *physics.CharacterController, tr *transform.Transform) {
		if cc.Every <= 1 {
			return
		}
		for _, car := range s.cars {
			if pedestrianInReach(car, tr.Translation, cc, dt) {
				s.every[e] = cc.Every
				cc.Every = 1
				break
			}
		}
	})
}

// Enclose the chassis about its root in every orientation.
func chassisRadius(spec *Spec) float32 {
	if spec == nil {
		return 3
	}
	b := spec.bounds()
	extent := rl.Vector3{X: max(-b.Min.X, b.Max.X), Y: max(-b.Min.Y, b.Max.Y), Z: max(-b.Min.Z, b.Max.Z)}
	return rl.Vector3Length(extent)
}

func pedestrianInReach(car movingChassis, at rl.Vector3, cc *physics.CharacterController, dt float32) bool {
	// Look ahead through the pedestrian's entire normal update interval,
	// plus one tick. This catches fast approaches and reversing too.
	delta := rl.Vector3Scale(car.velocity, max(0, dt)*float32(cc.Every+1))
	to := rl.Vector3Subtract(at, car.at)
	t := float32(0)
	if length := rl.Vector3LengthSqr(delta); length > 0 {
		t = max(0, min(1, rl.Vector3DotProduct(to, delta)/length))
	}
	separation := rl.Vector3Subtract(to, rl.Vector3Scale(delta, t))
	// Two metres of clearance wakes the controller before actual contact.
	reach := car.radius + max(cc.Height/2, cc.Radius) + 2
	return rl.Vector3LengthSqr(separation) <= reach*reach
}

func restorePedestrianSteps(people *illusion.Query1[physics.CharacterController], state *illusion.Res[pedestrianPhysics]) {
	s := state.Get()
	for e, every := range s.every {
		if cc, ok := people.Get(e); ok {
			cc.Every = every
		}
	}
	clear(s.every)
}
