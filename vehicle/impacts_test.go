package vehicle

import (
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

func TestImpactThresholds(t *testing.T) {
	r := ImpactRules{CriticalSpeed: 10 / 3.6, LethalSpeed: 30 / 3.6}
	for _, tt := range []struct {
		speed float32
		state character.LifeState
	}{
		{0, character.Healthy}, {r.CriticalSpeed - .001, character.Healthy},
		{r.CriticalSpeed, character.Critical}, {r.LethalSpeed - .001, character.Critical},
		{r.LethalSpeed, character.Dead}, {20, character.Dead},
	} {
		if got := r.condition(tt.speed); got != tt.state {
			t.Errorf("speed %v: state %v, want %v", tt.speed, got, tt.state)
		}
	}
}

func TestVehiclesRunOverCharacters(t *testing.T) {
	for _, tt := range []struct {
		name   string
		speed  float32
		state  character.LifeState
		player bool
	}{{"slow bump", 2, character.Healthy, false}, {"critical", 4, character.Critical, false},
		{"fatal", 12, character.Dead, false}, {"fatal reverse", -12, character.Dead, false},
		{"player critical", 4, character.Critical, true}, {"player fatal", 12, character.Dead, true}} {
		t.Run(tt.name, func(t *testing.T) {
			g := newRig(t)
			g.tap(rl.KeyE)
			g.tick(60)
			direction := float32(1)
			if tt.speed < 0 {
				direction = -1
			}
			person := pedestrian(g, rl.Vector3{Z: direction * 2.35}, 6)
			if tt.player {
				var cmd illusion.Commands
				cmd.InitParam(g.app.World)
				cmd.Entity(g.player).Remove(ecs.C[character.Player]())
				cmd.Entity(person).Insert(illusion.C(character.Player{}))
				g.tick(1)
			}
			get[physics.Velocity](g, g.car).Linear = rl.Vector3{Z: tt.speed}
			for range 25 {
				g.tick(1)
				if get[character.Health](g, person).State != character.Healthy {
					break
				}
			}
			health := *get[character.Health](g, person)
			if health.State != tt.state {
				t.Fatalf("impact %.1f km/h: health=%+v, want state %v", tt.speed*3.6, health, tt.state)
			}
			if tt.state == character.Healthy {
				return
			}
			if health.Vehicle != g.car {
				t.Fatal("impact must retain its source vehicle")
			}
			if ecs.NewMap[physics.CharacterController](g.app.World).Has(person) {
				t.Fatal("an incapacitated character still has an immovable standing capsule")
			}
			if *get[physics.RigidBody](g, person) != physics.Dynamic || *get[physics.Mass](g, person) != 70 {
				t.Fatal("victim must become a finite-mass fallen body")
			}
			if get[character.Intent](g, person).Move != (rl.Vector3{}) {
				t.Fatal("incapacitated character still walks")
			}
			if get[character.Health](g, g.player).State != character.Healthy {
				t.Fatal("driver injured by their own vehicle")
			}
			g.tick(60)
			at := get[transform.Transform](g, g.car).Translation
			if at.Z*direction < 3 {
				t.Fatalf("vehicle stopped on the victim: %v", at)
			}
			if get[physics.Velocity](g, g.car).Linear.Z*direction < tt.speed*direction*.3 {
				t.Fatal("impact removed the vehicle's momentum")
			}
		})
	}
}

func TestPassingVehicleDoesNotDamageNearbyCharacter(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	g.tick(60)
	person := pedestrian(g, rl.Vector3{X: 2, Z: 2.35}, 6)
	get[physics.Velocity](g, g.car).Linear = rl.Vector3{Z: 12}
	g.tick(60)
	if health := get[character.Health](g, person); health.State != character.Healthy {
		t.Fatalf("a nearby character was injured without chassis contact: %+v", health)
	}
}

func TestWallProtectsCharacterFromVehicle(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	person := pedestrian(g, rl.Vector3{Z: 8}, 6)
	var cmd illusion.Commands
	cmd.InitParam(g.app.World)
	cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(8, 4, 1)), illusion.C(transform.FromXYZ(0, 2, 4)))
	g.tick(2)
	get[physics.Velocity](g, g.car).Linear = rl.Vector3{Z: 12}
	g.tick(90)
	if get[character.Health](g, person).State != character.Healthy {
		t.Fatal("vehicle damaged a character through a wall")
	}
	if get[transform.Transform](g, g.car).Translation.Z > 4 {
		t.Fatal("vehicle drove through the wall")
	}
}
