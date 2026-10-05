package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
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
	// stairCycle is how far up a stair clip's cycle climbs: two steps, a
	// foot on each. The kit's stairs are twelve 0.3 m steps up a 3.6 m
	// deck (tools/world/hull_kit.py), so a deck is six whole cycles.
	stairCycle = 0.6
	// A slope counts as stairs from stairsFrom to stairsTo off level (the
	// stairs' ramp is 33°), and it stays stairs for stairsHold after.
	stairsFrom = 18 * math.Pi / 180
	stairsTo   = 50 * math.Pi / 180
	stairsHold = .15
	// fallFade is how long the fall's leg swing takes to fade in out of the
	// jump's pose.
	fallFade = .3
	// rollRun is the share of the run clip a roll or slide runs on into:
	// the left foot planted ahead and the right behind, as both leave them.
	rollRun = .25
)

// rollKeys retimes the roll's clip: pairs of how far through the roll
// (0 to 1) and how far through its clip. It lingers on the dive, the body
// stretched out forward in the air, and hurries through the tuck, so it
// reads as a dive and roll rather than a ball. The clip's parts: the take-off
// to .09, the dive to .3, the tuck to .55 and getting up after.
var rollKeys = [][2]float32{{0, 0}, {.1, .09}, {.4, .3}, {.55, .55}, {1, 1}}

// rollClip is how far through the roll's clip to show u of the way through
// the roll.
func rollClip(u float32) float32 {
	u = clamp(u, 0, 1)
	for i := 1; i < len(rollKeys); i++ {
		a, b := rollKeys[i-1], rollKeys[i]
		if u <= b[0] {
			return a[1] + (b[1]-a[1])*(u-a[0])/(b[0]-a[0])
		}
	}
	return 1
}

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

// onStairs is which way a character standing on ground with normal n and
// going at velocity v is going on stairs: 1 up, -1 down, 0 not on any (on
// the level, too steep, or not going anywhere).
func onStairs(n, v rl.Vector3) int8 {
	slope := math.Acos(float64(clamp(n.Y, -1, 1)))
	if slope < stairsFrom || slope > stairsTo || math.Hypot(float64(v.X), float64(v.Z)) < moving {
		return 0
	}
	// The normal leans downhill.
	if v.X*n.X+v.Z*n.Z < 0 {
		return 1
	}
	return -1
}

// stairsAnim is the clip for going dir on stairs (1 up, -1 down), or Idle
// for none.
func stairsAnim(dir int8) Anim {
	switch dir {
	case 1:
		return StairsUp
	case -1:
		return StairsDown
	}
	return Idle
}

// stairPhase is how far through its cycle a stair clip is, 0 to 1, with
// the feet at height y, going up or down: a cycle a stairCycle, a foot
// planted at step's share of it on each step's edge.
func stairPhase(y float32, up bool, step float32) float32 {
	climbed := y / stairCycle
	if !up {
		climbed = -climbed
	}
	p := climbed + step
	return p - float32(math.Floor(float64(p)))
}

// canAct reports whether a character can start an action: on the ground,
// standing or on the move, and not in the middle of one already.
func canAct(m motion, acting bool) bool {
	return m.Grounded && !acting
}

// action is the one-shot a body plays when asked for act on skin: a punch
// is with whichever hand's turn it is, the left if the skin has no right.
func (st *State) action(act Anim, skin *Skin) Anim {
	if act == Punch && st.rightPunch && skin.Has(PunchRight) {
		return PunchRight
	}
	return act
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

// duration is how long skin's clip name lasts, if it has it.
func (s *skins) duration(skin *Skin, name string) (float32, bool) {
	a := s.anims.Get().Get(skin.Anims)
	if a == nil {
		return 0, false
	}
	i, ok := a.Clip(name)
	if !ok {
		return 0, false
	}
	return a.Duration(i), true
}

// released reports whether p, playing an action on skin, is past release:
// far enough through that a character wanting to move can move on.
func (s *skins) released(skin *Skin, p *render.AnimationPlayer) bool {
	d, ok := s.duration(skin, p.Clip())
	return ok && p.Time() >= release*d
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
					case Roll:
						if traversal.Duration > 0 {
							p.Seek(rollClip(traversalTime(traversal, fixed.Get())/traversal.Duration) * duration)
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
			act := st.action(in.Act, skin)
			if canAct(m, showing) && skin.Has(act) {
				st.Current, acting = act, true
				play(p, skin.clip(act), true)
				if m.Speed > moving {
					p.FadeIn(fade) // out of a stride: ease into it as it pulls up
				}
				if act == Punch || act == PunchRight {
					st.rightPunch = act == Punch
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

		// On stairs, its walk or run climbs (or comes down) them a foot on
		// each step, if it has the clips.
		if dir := onStairs(cc.GroundNormal, cc.Velocity); cc.Grounded && dir != 0 {
			st.stairs, st.stairsLeft = dir, stairsHold
		} else if st.stairsLeft = max(0, st.stairsLeft-dt); st.stairsLeft == 0 {
			st.stairs = 0
		}
		next := pickAnim(m, *c, st.Current, acting)
		if stairs := stairsAnim(st.stairs); stairs != Idle && skin.Has(stairs) && (next == Walk || next == Run) {
			next = stairs
		}
		if next != st.Current {
			landing := st.Current.Airborne() && m.Grounded
			rolled := st.Current == Roll || st.Current == Slide
			st.Current = next
			clip := skin.clip(next)
			play(p, clip, next.Airborne() && !clip.Loop)
			if landing {
				p.FadeIn(.12)
			}
			if rolled && next == Run {
				// Running on out of a roll or slide: into the stride its feet
				// are in.
				if d, ok := sk.duration(skin, clip.Name); ok {
					p.Seek(rollRun * d)
				}
			}
		}

		// Match the stride to the ground speed, so feet don't slide, and a
		// jump's clip to its airtime.
		switch st.Current {
		case StairsUp, StairsDown:
			// Set by how high the feet are, so a foot lands on each step
			// whatever the pace.
			if d, ok := sk.duration(skin, skin.clip(st.Current).Name); ok {
				feet := samples.Position(samples.current, fixed.Get().Overstep()).Y - cc.Height/2
				p.ManualTime = true
				p.Seek(stairPhase(feet, st.Current == StairsUp, skin.clip(st.Current).Step) * d)
			}
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
					if skin.Has(Fall) {
						// Still falling: swing the legs until it lands.
						st.Current = Fall
						play(p, skin.clip(Fall), false)
						p.FadeIn(fallFade)
					} else {
						p.Paused = true // hold just before touchdown
					}
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
