package character

import (
	"math"

	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
)

const (
	// moving is the speed below which a character counts as standing.
	moving = 0.3
	// coyote is how long a character can be off the ground, stepping down a
	// curb say, before it looks airborne.
	coyote = 0.12
	fade   = 0.2
)

// motion is what the animation state machine knows about a character.
type motion struct {
	Grounded bool
	Speed    float32 // horizontal
}

// pickAnim chooses what a body should play. acting reports whether the
// current one-shot is still playing.
func pickAnim(m motion, c Character, current Anim, acting bool) Anim {
	switch {
	case !m.Grounded:
		if current.Airborne() {
			return current // a jump plays through, whatever the speed does
		}
		if m.Speed > moving {
			return RunJump
		}
		return Jump
	case m.Speed > moving:
		if m.Speed > (c.WalkSpeed+c.RunSpeed)/2 {
			return Run
		}
		return Walk
	case current.OneShot() && acting:
		return current
	}
	return Idle
}

// animate runs the state machine for every body, playing its skin's clip for
// its state. It runs in Update, before render.Animate advances the players.
func animate(
	bodies *illusion.Query2Where[State, render.AnimationPlayer, illusion.With[Body]],
	roots *illusion.Query3[Character, Intent, physics.CharacterController],
	hier *illusion.Hierarchy,
	roster *illusion.Res[Roster],
	settings *illusion.Res[physics.Settings],
	t *illusion.Res[illusion.Time],
) {
	skins := roster.Get().Skins
	gravity := -settings.Get().Gravity.Y
	dt := t.Get().DeltaSecs()
	bodies.Each(func(e ecs.Entity, st *State, p *render.AnimationPlayer) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		c, in, cc, ok := roots.Get(parent)
		if !ok {
			return
		}
		skin := &skins[st.skin]
		if cc.Grounded {
			st.air = 0
		} else {
			st.air += dt
		}
		m := motion{
			// A jump shows at once; a step down only after a moment.
			Grounded: cc.Grounded || st.air < coyote && cc.Velocity.Y <= 0,
			Speed:    float32(math.Hypot(float64(cc.Velocity.X), float64(cc.Velocity.Z))),
		}

		acting := st.Current.OneShot() && !p.Finished()
		if in.Act != Idle {
			if m.Grounded && m.Speed <= moving && skin.Has(in.Act) {
				st.Current, acting = in.Act, true
				play(p, skin.clip(in.Act), true)
			}
			in.Act = Idle
		}

		if next := pickAnim(m, *c, st.Current, acting); next != st.Current {
			st.Current = next
			clip := skin.clip(next)
			play(p, clip, next.Airborne() && !clip.Loop)
		}

		// Match the stride to the ground speed, so feet don't slide, and a
		// jump's clip to its airtime.
		switch st.Current {
		case Walk:
			p.Speed = clamp(m.Speed/c.WalkSpeed, 0.6, 1.6)
		case Run:
			p.Speed = clamp(m.Speed/c.RunSpeed, 0.7, 1.4)
		case Jump, RunJump:
			clip := skin.clip(st.Current)
			switch {
			case clip.Hold:
				// Seeking back each frame, rather than pausing, holds the
				// pose and lets the crossfade into it finish.
				p.Seek(clip.Start)
			case clip.Loop:
				p.Speed = 1
			default:
				p.Speed = airSpeed(clip, c.JumpSpeed, gravity)
				if clip.Land > 0 && p.Time() >= clip.Land {
					p.Paused = true // still falling: hold just before touchdown
				}
			}
		default:
			p.Speed = 1
		}
	})
}

// play starts clip, crossfading from whatever was playing. Once-clips hold
// their last frame; starting the one already playing starts it over.
func play(p *render.AnimationPlayer, clip Clip, once bool) {
	p.Paused = false
	switch {
	case !once:
		p.Play(clip.Name).FadeIn(fade)
	case p.Clip() == clip.Name:
		p.Replay()
	default:
		p.PlayOnce(clip.Name).FadeIn(fade / 2)
	}
	if clip.Start > 0 {
		p.Seek(clip.Start)
	}
}

// airSpeed is the playback speed that stretches a jump clip's take-off to
// landing over the airtime of a jump at jumpSpeed.
func airSpeed(clip Clip, jumpSpeed, gravity float32) float32 {
	if clip.Land <= clip.Start || jumpSpeed <= 0 || gravity <= 0 {
		return 1
	}
	airtime := 2 * jumpSpeed / gravity
	return clamp((clip.Land-clip.Start)/airtime, 0.25, 2)
}

func clamp(v, lo, hi float32) float32 { return max(lo, min(v, hi)) }
