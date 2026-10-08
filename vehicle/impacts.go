package vehicle

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// ImpactRules are gameplay thresholds in metres per second. Defaults:
// critical at 10 km/h, instant death at 30 km/h of closing speed.
type ImpactRules struct{ CriticalSpeed, LethalSpeed float32 }

func (r *ImpactRules) condition(speed float32) character.LifeState {
	if speed >= r.LethalSpeed {
		return character.Dead
	}
	if speed >= r.CriticalSpeed {
		return character.Critical
	}
	return character.Healthy
}

type impactPeople struct {
	roots       illusion.Query2[character.Character, transform.Transform]
	controllers illusion.Query1[physics.CharacterController]
	health      illusion.Query1[character.Health]
	velocities  illusion.Query1[physics.Velocity]
	seated      illusion.Query1[character.Seated]
}

func (p *impactPeople) InitParam(w *ecs.World) {
	p.roots.InitParam(w)
	p.controllers.InitParam(w)
	p.health.InitParam(w)
	p.velocities.InitParam(w)
	p.seated.InitParam(w)
}

type impactCar struct {
	at, velocity, angular rl.Vector3
	driver                ecs.Entity
	radius                float32
}

func (c impactCar) velocityAt(at rl.Vector3) rl.Vector3 {
	return rl.Vector3Add(c.velocity, rl.Vector3CrossProduct(c.angular, rl.Vector3Subtract(at, c.at)))
}

type impactMemory struct {
	cars   map[ecs.Entity]impactCar
	people map[ecs.Entity]rl.Vector3
	hits   map[ecs.Entity]vehicleImpact
}

type vehicleImpact struct {
	vehicle  ecs.Entity
	speed    float32
	velocity rl.Vector3
	at       transform.Transform
}

// Predict actual capsule/chassis contact before the solver sees an upright
// infinite-mass character. Use a shape sweep, not a proximity damage radius.
func runOver(
	cmd *illusion.Commands,
	cars *illusion.Query4[Drivable, physics.RigidBody, transform.Transform, physics.Velocity],
	people *impactPeople,
	p *physics.Physics,
	rules *illusion.Res[ImpactRules],
	state *illusion.Res[impactMemory],
	settings *illusion.Res[physics.Settings],
	clock *illusion.Res[illusion.Time],
) {
	s := state.Get()
	clear(s.cars)
	clear(s.people)
	clear(s.hits)
	if settings.Get().Paused {
		return
	}
	cars.Each(func(e ecs.Entity, car *Drivable, body *physics.RigidBody, tr *transform.Transform, velocity *physics.Velocity) {
		if *body == physics.Dynamic {
			s.cars[e] = impactCar{at: tr.Translation, velocity: velocity.Linear, angular: velocity.Angular, driver: car.Driver, radius: chassisRadius(car.Spec)}
		}
	})
	if len(s.cars) == 0 {
		return
	}
	dt := clock.Get().DeltaSecs()
	people.roots.Each(func(e ecs.Entity, _ *character.Character, tr *transform.Transform) {
		health := character.Health{}
		if h, ok := people.health.Get(e); ok {
			health = *h
		}
		if health.State == character.Dead || people.seated.Contains(e) {
			return
		}
		cc, standing := people.controllers.Get(e)
		velocity := rl.Vector3{}
		if standing {
			velocity = cc.Velocity
		} else if v, ok := people.velocities.Get(e); ok {
			velocity = v.Linear
		}
		s.people[e] = velocity
		if !standing {
			return
		} // fallen bodies use genuine contact events below
		radius := cc.Radius
		if radius <= 0 {
			radius = .3
		}
		height := max(cc.Height, 2*radius+.01)
		for vehicle, car := range s.cars {
			if car.driver == e {
				continue
			}
			motion := car.velocityAt(tr.Translation)
			if rl.Vector3Length(motion) < rules.Get().CriticalSpeed {
				continue
			}
			relative := rl.Vector3Subtract(motion, velocity)
			distance := rl.Vector3Length(relative)
			if distance < rules.Get().CriticalSpeed {
				continue
			}
			// A cheap bound avoids shape queries across the whole world.
			reach := car.radius + height/2 + distance*dt + .05
			if rl.Vector3DistanceSqr(car.at, tr.Translation) > reach*reach {
				continue
			}
			delta := rl.Vector3Scale(relative, -(dt + .04/max(distance, .001)))
			// Raise by a small skin so the standing floor doesn't mask the chassis.
			origin := rl.Vector3Add(tr.Translation, rl.Vector3{Y: .025})
			hit, ok := p.SweepCapsuleExcluding(origin, delta, radius, height, e)
			if !ok || hit.Entity != vehicle {
				continue
			}
			speed := max(0, rl.Vector3DotProduct(relative, hit.Normal))
			if rules.Get().condition(speed) == character.Healthy {
				continue
			}
			if old, ok := s.hits[e]; ok && old.speed >= speed {
				continue
			}
			s.hits[e] = vehicleImpact{vehicle: vehicle, speed: speed, velocity: motion, at: *tr}
		}
	})
	for e, hit := range s.hits {
		applyVehicleImpact(cmd, people, rules.Get(), e, hit)
	}
}

func applyVehicleImpact(cmd *illusion.Commands, people *impactPeople, rules *ImpactRules, e ecs.Entity, hit vehicleImpact) {
	status := rules.condition(hit.speed)
	if status == character.Healthy {
		return
	}
	if h, ok := people.health.Get(e); ok && h.State >= status {
		return
	}
	health := character.Health{State: status, ImpactSpeed: hit.speed, Vehicle: hit.vehicle}
	cmd.Queue(func(w *ecs.World) {
		if driving := ecs.GetResource[Driving](w); driving != nil && driving.Vehicle == hit.vehicle {
			text := "Character critically injured"
			if status == character.Dead {
				text = "Fatal vehicle impact"
			}
			if prompt := ecs.GetResource[Prompt](w); prompt != nil {
				prompt.note(text)
			}
		}
	})
	if people.controllers.Contains(e) {
		// Carry some impact momentum into the finite-mass fallen body.
		velocity := rl.Vector3Scale(hit.velocity, .6)
		velocity.Y += min(3, hit.speed*.15)
		character.KnockDown(cmd.Entity(e), hit.at, health, velocity)
	} else {
		cmd.Entity(e).Insert(illusion.C(health))
	}
}

// Contact fallback also handles another vehicle running over a critical
// character. Speeds come from before the solver, not its post-impact result.
func impactContacts(
	events *illusion.EventReader[physics.CollisionStarted],
	people *impactPeople,
	state *illusion.Res[impactMemory],
	rules *illusion.Res[ImpactRules],
	cmd *illusion.Commands,
) {
	s := state.Get()
	for event := range events.Read() {
		vehicle, actor, normal := event.A, event.B, event.Normal
		car, ok := s.cars[vehicle]
		if !ok {
			vehicle, actor, normal = event.B, event.A, rl.Vector3Negate(normal)
			car, ok = s.cars[vehicle]
		}
		if !ok || car.driver == actor {
			continue
		}
		velocity, isPerson := s.people[actor]
		_, tr, exists := people.roots.Get(actor)
		if !isPerson || !exists {
			continue
		}
		motion := car.velocityAt(event.Point)
		if rl.Vector3Length(motion) < rules.Get().CriticalSpeed {
			continue
		}
		speed := max(0, rl.Vector3DotProduct(rl.Vector3Subtract(motion, velocity), normal))
		applyVehicleImpact(cmd, people, rules.Get(), actor, vehicleImpact{vehicle: vehicle, speed: speed, velocity: motion, at: *tr})
	}
}
