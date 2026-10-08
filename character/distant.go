package character

import (
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
)

// Distant on a character's body marks it as far off, which the game decides
// (it knows where the camera is). A distant character is posed in step with
// every other distant one playing the same clip, and isn't fitted to the
// ground under its feet: raylib skins on the CPU, and a mesh posed the same
// as it was last drawn isn't skinned again, so a crowd far off costs a
// skinning for each of the models it's made of, not for each person in it.
// At a distance nobody sees that they move together.
type Distant struct{}

// lockstep poses every distant body at one of a few moments of its clip:
// the time since the start, which every looping clip wraps round, put off by
// one of lockstepPhases (by which body it is, so it keeps to it), with no
// crossfade (it changes clips at once), and with no pose of its own
// (fitPoseToWorld's). The phases keep a crowd from
// walking all in step; there are few, so each mesh is still skinned only a
// few times.
func lockstep(
	bodies *illusion.Query2Where[render.AnimationPlayer, State, illusion.And[illusion.With[Body], illusion.With[Distant]]],
	clock *illusion.Res[illusion.Time],
) {
	t := clock.Get().ElapsedSecs()
	bodies.Each(func(e ecs.Entity, p *render.AnimationPlayer, state *State) {
		if state.downed {
			return
		}
		p.Seek(t + lockstepPhases[int(e.ID())%len(lockstepPhases)])
		p.Settle()
		p.Pose = nil
	})
}

// lockstepPhases are the moments distant bodies are put off by, in seconds:
// apart in any clip about a second long, as walks and runs are.
var lockstepPhases = [...]float32{0, .53}
