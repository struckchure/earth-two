package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
)

// TraversalConfig belongs to each character, not the resource store. Query it
// alongside that player's movement so sprinting uses the same rules as physics.
type touchPlayers = illusion.Query4Where[character.Traversal, character.Intent, physics.CharacterController, character.TraversalConfig, illusion.With[character.Player]]

func touchMovement(players *touchPlayers) (running, sliding bool) {
	players.Each(func(_ ecs.Entity, _ *character.Traversal, in *character.Intent, cc *physics.CharacterController, cfg *character.TraversalConfig) {
		speed := float32(math.Hypot(float64(cc.Velocity.X), float64(cc.Velocity.Z)))
		running = in.Run && in.Move != (rl.Vector3{}) && cc.Grounded && speed > 0.2
		sliding = running && speed >= cfg.SlideMin
	})
	return
}
