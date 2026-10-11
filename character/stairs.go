package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// StairSurface marks authored staircase collision geometry. A sloped normal
// alone cannot distinguish stairs from hills, dunes, roofs or ordinary ramps.
type StairSurface struct{}

func detectStairs(bodies *illusion.Query1Where[State, illusion.With[Body]], roots *illusion.Query2[physics.CharacterController, transform.Transform], hierarchy *illusion.Hierarchy, world *physics.Physics, stairs *illusion.Query1[StairSurface], clock *illusion.Res[illusion.Time]) {
	dt := clock.Get().DeltaSecs()
	bodies.Each(func(body ecs.Entity, state *State) {
		parent, ok := hierarchy.Parent(body)
		if !ok {
			return
		}
		cc, at, ok := roots.Get(parent)
		if !ok {
			state.stairs, state.stairsLeft = 0, 0
			return
		}
		direction := onStairs(cc.GroundNormal, cc.Velocity)
		if cc.Grounded && direction != 0 {
			hit, found := world.CastRayExcluding(at.Translation, rl.Vector3{Y: -1}, cc.Height/2+max(cc.StepHeight, .2)+.05, parent)
			if found && stairs.Contains(hit.Entity) {
				state.stairs, state.stairsLeft = direction, stairsHold
			} else {
				// Never carry stair speed/animation onto an ordinary inclined surface.
				state.stairs, state.stairsLeft = 0, 0
			}
		} else if state.stairsLeft = max(0, state.stairsLeft-dt); state.stairsLeft == 0 {
			state.stairs = 0
		}
	})
}
