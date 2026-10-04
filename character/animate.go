package character

import (
	"math"

	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
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
	// release is the share of an action's clip after which a character that
	// wants to move stops acting and gets going, skipping the recovery back
	// to standing; resume is how long, leaving one, it counts as walking or
	// running while it gets back up to speed.
	release = 0.7
	resume  = 0.3
	// pivot is how fast (radians per second) a body turns before it steps
	// round rather than swivelling on the spot.
	pivot = 1
)

// motion is what the animation state machine knows about a character.
type motion struct {
	Grounded bool
	Speed    float32 // horizontal
	// Resuming is set just after an action while the character wants to
	// move on, and Pivoting while it turns about to head somewhere, too
	// slow to count as moving: both step, rather than stand.
	Resuming, Pivoting bool
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
	case current.OneShot() && acting:
		return current // it stands still for it (see locomote); only a fall cuts it short
	case m.Speed > moving:
		if m.Speed > (c.WalkSpeed+c.RunSpeed)/2 {
			return Run
		}
		return Walk
	case m.Resuming || m.Pivoting:
		// Straight from the action into a walk as it speeds back up (into a
		// run, if it's running, once it's going fast enough); or stepping
		// round on the spot.
		return Walk
	}
	return Idle
}

// canAct reports whether a character can start an action: on the ground,
// standing or on the move, and not in the middle of one already.
func canAct(m motion, acting bool) bool {
	return m.Grounded && !acting
}

// skins is the roster's skins and their clips, for animate (one parameter,
// to stay within Fn6).
type skins struct {
	roster illusion.Res[Roster]
	anims  illusion.Res[asset.Assets[render.Animations]]
}

func (s *skins) InitParam(w *ecs.World) {
	s.roster.InitParam(w)
	s.anims.InitParam(w)
}

// released reports whether p, playing an action on skin, is past release:
// far enough through that a character wanting to move can move on.
func (s *skins) released(skin *Skin, p *render.AnimationPlayer) bool {
	a := s.anims.Get().Get(skin.Anims)
	if a == nil {
		return false
	}
	i, ok := a.Clip(p.Clip())
	return ok && p.Time() >= release*a.Duration(i)
}

// animate runs the state machine for every body, playing its skin's clip for
// its state. It runs in Update, before render.Animate advances the players.
func animate(
	bodies *illusion.Query2Where[State, render.AnimationPlayer, illusion.With[Body]],
	roots *illusion.Query5[Character, Intent, physics.CharacterController, Traversal, MotionSamples],
	hier *illusion.Hierarchy,
	sk *skins,
	settings *illusion.Res[physics.Settings],
	t *illusion.Res[illusion.Time],
	controls *illusion.Res[Controls],
	fixed *illusion.Res[illusion.FixedTime],
) {
	skins := sk.roster.Get().Skins
	gravity := -settings.Get().Gravity.Y
	dt := t.Get().DeltaSecs()
	bodies.Each(func(e ecs.Entity, st *State, p *render.AnimationPlayer) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		c, in, cc, traversal, samples, ok := roots.Get(parent)
		if !ok {
			return
		}
		skin := &skins[st.skin]
		p.ManualTime = false
		if !traversal.active() && traversal.Kick == 0 && !cc.Grounded && st.Current >= WallKick && st.Current <= WallFallRight {
			// After a wall kick it falls as the kick left it, loosely, until
			// it lands.
			p.Paused = !controls.Get().Enabled || settings.Get().Paused
			if p.Paused {
				return
			}
			next := WallFall
			if st.Current == WallKickRight || st.Current == WallFallRight {
				next = WallFallRight
			}
			if next != st.Current {
				st.Current = next
				play(p, skin.clip(next), false)
				p.FadeIn(.2)
			}
			in.Act = Idle
			p.Speed = 1
			return
		}
		if traversal.active() || traversal.Kick > 0 {
			if !controls.Get().Enabled || settings.Get().Paused {
				p.Paused = true
				return
			}
			p.Paused = false
			p.ManualTime = true
			next := traversal.Mode
			if !traversal.active() {
				next = WallKick
				if traversal.KickRight {
					next = WallKickRight
				}
			}
			if next != st.Current {
				st.Current = next
				play(p, skin.clip(next), next != LadderClimb && next != LadderEnter && next != Crouch)
				p.FadeIn(.22)
				if next == Slide || next == Roll {
					p.FadeIn(.09)
				}
				if next == WallKick || next == WallKickRight {
					p.FadeIn(.07)
				}
				if next == StandUp {
					p.FadeIn(.08)
				}
				if next == LadderExit && !traversal.ExitTop {
					p.FadeIn(.12)
				}
				if next == LadderClimb {
					p.FadeIn(.12)
				}
			}
			in.Act = Idle
			st.turn = 0
			p.Speed = 1
			a := sk.anims.Get().Get(skin.Anims)
			if a != nil {
				if i, ok := a.Clip(skin.clip(next).Name); ok {
					duration := a.Duration(i)
					switch next {
					case WallKick, WallKickRight:
						elapsed := wallKickTime - traversal.Kick + (fixed.Get().Overstep()-1)*float32(fixed.Get().Timestep.Seconds())
						p.Seek(clamp(elapsed/wallKickTime, 0, 1) * duration)
					case LadderClimb:
						spacing := max(.01, traversal.RungSpacing)
						phase := traversal.Phase + cc.Walk.Y*fixed.Get().Overstep()*float32(fixed.Get().Timestep.Seconds())/(2*spacing)
						phase -= float32(math.Floor(float64(phase)))
						p.Seek(phase * duration)
					case LadderEnter:
						phase := traversal.Phase - float32(math.Floor(float64(traversal.Phase)))
						p.Seek(phase * duration)
					case Crouch:
						p.Seek(0)
					case LadderExit:
						if !traversal.ExitTop {
							p.Seek(duration)
						} else {
							p.Seek(traversalTime(traversal, fixed.Get()) / traversal.Duration * duration)
						}
					default:
						if traversal.Duration > 0 && next != WallKick {
							p.Seek(traversalTime(traversal, fixed.Get()) / traversal.Duration * duration)
						}
					}
				}
			}
			return
		}

		if cc.Grounded {
			st.air = 0
		} else {
			st.air += dt
		}
		m := motion{
			// A jump shows at once; a step down only after a moment.
			Grounded: !controls.Get().Enabled || cc.Grounded || st.air < coyote && cc.Velocity.Y <= 0,
			Speed:    float32(math.Hypot(float64(cc.Velocity.X), float64(cc.Velocity.Z))),
		}
		// Against a wall it wants to go but doesn't: it stands, rather than
		// walking on the spot. (The lesser of the two, as a teleport is no
		// speed at all.)
		m.Speed = min(m.Speed, samples.Speed(float32(fixed.Get().Timestep.Seconds())))

		heading := in.Move.X != 0 || in.Move.Z != 0
		// Landing from a wall kick it sinks into its knees, unless it's
		// still sprinting: then it runs straight on.
		if st.Current >= WallKick && st.Current <= WallFallRight && cc.Grounded && skin.Has(WallLand) && !(in.Run && heading) {
			st.Current = WallLand
			play(p, skin.clip(WallLand), true)
			p.FadeIn(.08)
		}
		if st.Current == WallLand && m.Grounded && !p.Finished() && (!heading || traversal.Land > 0) {
			// Taking the landing from a wall kick; wanting to move cuts it
			// short, once it's free to (see TraversalConfig.LandDelay).
			p.Speed = 1
			return
		}
		acting := st.Current.OneShot() && !p.Finished()
		if acting && heading && sk.released(skin, p) {
			acting = false // wants to move on: skip the recovery
		}
		if in.Act != Idle {
			// Moving on, a released action is still on screen this frame:
			// starting one now would jump its clip back to the start.
			showing := st.Current.OneShot() && !p.Finished()
			if canAct(m, showing) && skin.Has(in.Act) {
				st.Current, acting = in.Act, true
				play(p, skin.clip(in.Act), true)
				if m.Speed > moving {
					p.FadeIn(fade) // out of a stride: ease into it as it pulls up
				}
			}
			in.Act = Idle // started or not, it's done with
		}
		if st.Current.OneShot() && !acting && heading {
			st.resume = resume
		}
		st.resume = max(0, st.resume-dt)
		m.Resuming = heading && st.resume > 0
		m.Pivoting = heading && abs(st.turn) > pivot

		if next := pickAnim(m, *c, st.Current, acting); next != st.Current {
			landing := st.Current.Airborne() && m.Grounded
			st.Current = next
			clip := skin.clip(next)
			play(p, clip, next.Airborne() && !clip.Loop)
			if landing {
				p.FadeIn(.12)
			}
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

// Match the same previous/current interval used by the interpolated body.
func traversalTime(s *Traversal, fixed *illusion.FixedTime) float32 {
	return clamp(s.Elapsed+(fixed.Overstep()-1)*float32(fixed.Timestep.Seconds()), 0, s.Duration)
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
