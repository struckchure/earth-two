package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// wallKickTime matches the wall kick clip: one foot taps the wall and pushes
// off, and the body turns away from it in the air.
const wallKickTime float32 = 8. / 30

// rollRelease and slideRelease are the shares of a roll and a slide after
// which a character still sprinting runs straight on: by then its feet are
// under it in a stride, and the rest is getting up to stand. (The slide's
// clip gets up by itself; see SLIDE_FRAMES in tools/makehuman/traversal.py.)
const (
	rollRelease  = .6
	slideRelease = .875
)

// A slide goes the way the character was running, and steers at most
// slideSteer either side of that, turning at most slideTurn radians a
// second. Pulling back further than slideBack from the way it's going
// doesn't steer it: it can't turn round.
const (
	slideSteer = 45 * math.Pi / 180
	slideTurn  = 1.5
	slideBack  = 120 * math.Pi / 180
)

// steerSlide is the way a slide going along current, which started heading
// the yaw heading, goes after dt seconds of wanting to go move.
func steerSlide(current rl.Vector3, heading float32, move rl.Vector3, dt float32) rl.Vector3 {
	move = horizontal(move)
	if move == (rl.Vector3{}) {
		return current
	}
	yaw := float32(math.Atan2(float64(current.X), float64(current.Z)))
	to := float32(math.Atan2(float64(move.X), float64(move.Z)))
	if abs(wrapAngle(to-yaw)) > slideBack {
		return current
	}
	want := heading + clamp(wrapAngle(to-heading), -slideSteer, slideSteer)
	yaw += clamp(wrapAngle(want-yaw), -slideTurn*dt, slideTurn*dt)
	return rl.Vector3{X: float32(math.Sin(float64(yaw))), Z: float32(math.Cos(float64(yaw)))}
}

// TraversalConfig contains distances, seconds and speeds used by every character.
type TraversalConfig struct {
	LowHeight, SlideMin, SlideSpeed, SlideTime, RollDistance, RollTime float32
	LadderSpeed, KickSpeed, KickUp, Guard                              float32
	KickMin                                                            float32 // the slowest it can be going to kick off a wall: a sprint
	LandDelay                                                          float32 // seconds after landing from a wall kick before it moves off, unless it's still sprinting
	// How far the body's surface may be from a box, a ladder or a wall and
	// still use it: near enough that the hands and feet meet it.
	Reach, LadderReach, WallReach float32
}

func DefaultTraversal() TraversalConfig {
	return TraversalConfig{LowHeight: .9, SlideMin: 3, SlideSpeed: 6.5, SlideTime: .8, RollDistance: 2, RollTime: .7, LadderSpeed: .6, Reach: .3, LadderReach: .25, WallReach: .35, KickSpeed: 3.2, KickUp: 3.6, Guard: .15, KickMin: 4, LandDelay: .15}
}

// Traversal is authoritative fixed-step state; animations follow it.
type Traversal struct {
	Mode                             Anim
	Elapsed, Duration, Guard, Detach float32
	Direction                        rl.Vector3
	Speed                            float32
	Start, Lift, End                 rl.Vector3
	Ladder                           ecs.Entity
	LastWall                         ecs.Entity
	WallNormal                       rl.Vector3
	WallPoint                        rl.Vector3
	WallLocked                       bool
	Kick                             float32
	KickRight                        bool    // the wall is on the right: the right foot kicks
	Bounced                          bool    // in the air off a wall kick: it can't steer until it lands
	Land                             float32 // seconds left of LandDelay
	impulse                          bool
	queued                           float32 // seconds left to face an obstacle it's been asked to get over
	Hint                             string
	Phase, RungSpacing               float32 // climb cycles, driven by distance, not animation time
	ExitTop                          bool
	heading                          float32 // the way a slide started, as a yaw: it steers only so far from it
}

func (s *Traversal) active() bool { return s.Mode != Idle }

// Ladder is an authored static route. Bottom/Top are feet positions; Facing
// points toward its rungs. Exit positions must have standing clearance.
type Ladder struct {
	Bottom, Top, Facing, BottomExit, TopExit rl.Vector3
	Width                                    float32
	RungSpacing                              float32 // defaults to the authored clip spacing (0.3 metres)
}

func horizontal(v rl.Vector3) rl.Vector3 { v.Y = 0; return v }
func direction(v rl.Vector3) rl.Vector3 {
	if rl.Vector3LengthSqr(v) < .0001 {
		return rl.Vector3{}
	}
	return rl.Vector3Normalize(v)
}
func easeTraversal(u float32) float32 {
	u = clamp(u, 0, 1)
	return u * u * (3 - 2*u)
}

// The exit clip is authored against these (tools/makehuman/traversal.py):
// the shares of a top exit by which the body has climbed to within
// ladderExitHop of the landing, hopped up level with it, and crossed onto it;
// how long it takes; and how far it climbs on before the hop (ladderExitRise),
// which is how far below it starts. ladderFoot is how far up a ladder the
// climb's cycle starts, the left foot leaving its rung: the clip's feet are
// on the rungs for ladders whose first rung is 30 cm up, 30 cm apart.
const (
	ladderExitClimb, ladderExitUp, ladderExitCross = 42. / 90, 63. / 90, 81. / 90
	ladderExitHop                                  = .91
	ladderExitTime                                 = 3
	ladderExitRise, ladderFoot                     = .84, .21
)

// All of it at a steady pace: the clip's own motion is laid over the top.
func ladderExitPosition(s *Traversal, u float32) rl.Vector3 {
	if !s.ExitTop {
		return rl.Vector3Lerp(s.Start, s.End, easeTraversal(u))
	}
	hop := s.Lift
	hop.Y = max(s.Start.Y, hop.Y-ladderExitHop)
	switch {
	case u < ladderExitClimb:
		return rl.Vector3Lerp(s.Start, hop, u/ladderExitClimb)
	case u < ladderExitUp:
		return rl.Vector3Lerp(hop, s.Lift, (u-ladderExitClimb)/(ladderExitUp-ladderExitClimb))
	}
	return rl.Vector3Lerp(s.Lift, s.End, min(1, (u-ladderExitUp)/(ladderExitCross-ladderExitUp)))
}

func standUp(s *Traversal, cc *physics.CharacterController) {
	s.Mode, s.Elapsed, s.Duration = StandUp, 0, .28
	cc.Walk = rl.Vector3{}
}

func pathPosition(s *Traversal, u float32) rl.Vector3 {
	// Lift vertically before passing across the obstacle, then settle onto its top/landing.
	switch {
	case u < .4:
		return rl.Vector3Lerp(s.Start, s.Lift, u/.4)
	case u < .85:
		return rl.Vector3Lerp(s.Lift, rl.Vector3{X: s.End.X, Y: s.Lift.Y, Z: s.End.Z}, (u-.4)/.45)
	default:
		return rl.Vector3Lerp(rl.Vector3{X: s.End.X, Y: s.Lift.Y, Z: s.End.Z}, s.End, (u-.85)/.15)
	}
}
func clearPath(p *physics.Physics, e ecs.Entity, cc *physics.CharacterController, from, to rl.Vector3) bool {
	if p.OverlapCapsuleExcluding(to, cc.Radius, cc.Height, e) {
		return false
	}
	_, hit := p.SweepCapsuleExcluding(from, rl.Vector3Subtract(to, from), cc.Radius, cc.Height, e)
	return !hit
}
func endTraversal(s *Traversal, cc *physics.CharacterController) {
	s.Mode = Idle
	s.Elapsed = 0
	cc.Controlled = false
	cc.Walk = rl.Vector3{}
}
func refreshLaunch(st *State, cc *physics.CharacterController, dir rl.Vector3) {
	st.launch = horizontal(cc.Walk)
	st.launchYaw = float32(math.Atan2(float64(dir.X), float64(dir.Z)))
	st.aloft = true
}

func traverse(
	q *illusion.Query6[Character, Intent, physics.CharacterController, transform.Transform, Traversal, TraversalConfig],
	bodies *illusion.Query2Where[State, transform.Transform, illusion.With[Body]],
	ladders *illusion.Query1[Ladder], hier *illusion.Hierarchy, p *physics.Physics,
	t *illusion.Res[illusion.Time], controls *illusion.Res[Controls], settings *illusion.Res[physics.Settings],
) {
	dt := t.Get().DeltaSecs()
	if dt <= 0 || settings.Get().Paused || !controls.Get().Enabled {
		return
	}
	q.Each(func(e ecs.Entity, c *Character, in *Intent, cc *physics.CharacterController, tr *transform.Transform, s *Traversal, cfg *TraversalConfig) {
		s.impulse = false
		slide, roll, jump := in.Slide, in.Roll, in.Jump
		in.Slide, in.Roll = false, false
		s.Guard = max(0, s.Guard-dt)
		s.Detach = max(0, s.Detach-dt)
		s.Kick = max(0, s.Kick-dt)
		if cc.Grounded {
			s.Kick = 0
		}
		s.Land = max(0, s.Land-dt)
		s.queued = max(0, s.queued-dt)
		if s.Bounced && cc.Grounded {
			s.Land = cfg.LandDelay
			if in.Run && in.Move != (rl.Vector3{}) {
				s.Land = 0 // still sprinting: it runs straight on
			}
		}
		if cc.Grounded || s.active() {
			s.Bounced = false
		}
		s.Hint = ""
		var st *State
		var body *transform.Transform
		hier.EachChild(e, func(child ecs.Entity) {
			if a, b, ok := bodies.Get(child); ok {
				st, body = a, b
			}
		})
		facing := rl.Vector3{Z: 1}
		if body != nil {
			facing = rl.Vector3RotateByQuaternion(facing, body.Rotation)
		}
		dir := direction(horizontal(in.Move))
		if dir == (rl.Vector3{}) {
			dir = direction(facing)
		}
		if s.WallLocked && (cc.Grounded || rl.Vector3DotProduct(rl.Vector3Subtract(tr.Translation, s.WallPoint), s.WallNormal) > cc.Radius+cfg.WallReach+.1) {
			s.WallLocked = false
		}
		acting := st != nil && st.Current.OneShot()
		// Stay low until the full capsule fits; never grow through an overhead obstacle.
		if s.Mode == Crouch {
			if p.ResizeCharacter(e, cc, tr, capsuleHeight) {
				standUp(s, cc)
			} else {
				cc.Walk = rl.Vector3Scale(direction(in.Move), .8)
				in.Jump = false
				s.Hint = "Move out to stand"
				return
			}
		}
		if !s.active() && cc.Height < capsuleHeight {
			if !p.ResizeCharacter(e, cc, tr, capsuleHeight) {
				s.Mode = Crouch
				cc.Walk = rl.Vector3Scale(direction(in.Move), .8)
				in.Jump = false
				return
			}
		}
		if s.active() {
			in.Jump = false
			s.Elapsed += dt
			switch s.Mode {
			case StandUp:
				if roll && cc.Grounded && s.Guard == 0 && p.ResizeCharacter(e, cc, tr, cfg.LowHeight) {
					s.Mode, s.Elapsed, s.Duration = Roll, 0, cfg.RollTime
					s.Direction, s.Speed, s.Guard = dir, cfg.RollDistance/cfg.RollTime, cfg.Guard
					cc.Walk = rl.Vector3{}
					return
				}
				// Recover while moving, without a stationary lock after every slide.
				cc.Walk = rl.Vector3Scale(direction(in.Move), c.WalkSpeed*easeTraversal(s.Elapsed/s.Duration))
				if jump && cc.Grounded {
					launch := cc.Walk
					endTraversal(s, cc)
					cc.Walk, cc.Velocity.Y = launch, c.JumpSpeed
					s.impulse = true
					if st != nil {
						refreshLaunch(st, cc, dir)
					}
					return
				}
				if s.Elapsed >= s.Duration {
					walk := cc.Walk
					endTraversal(s, cc)
					cc.Walk = walk
				}
			case LadderEnter, LadderExit:
				cc.Controlled = true
				next := rl.Vector3Lerp(s.Start, s.End, easeTraversal(s.Elapsed/s.Duration))
				if s.Mode == LadderExit {
					next = ladderExitPosition(s, s.Elapsed/s.Duration)
				}
				if !clearPath(p, e, cc, tr.Translation, next) {
					endTraversal(s, cc)
					cc.Velocity = rl.Vector3{}
					s.Detach = .5
					return
				}
				cc.Walk = rl.Vector3Scale(rl.Vector3Subtract(next, tr.Translation), 1/dt)
				if s.Elapsed >= s.Duration {
					tr.Translation = s.End
					cc.Walk, cc.Velocity = rl.Vector3{}, rl.Vector3{}
					if s.Mode == LadderEnter {
						s.Mode, s.Elapsed = LadderClimb, 0
					} else {
						endTraversal(s, cc)
						s.Detach = .5
						if st != nil {
							st.air = 0
							st.aloft = false
						}
					}
				}
			case Slide, Roll:
				slideMode := s.Mode == Slide
				if !cc.Grounded {
					endTraversal(s, cc)
					return
				}
				if s.Mode == Slide && jump {
					launch := horizontal(cc.Walk)
					cc.Velocity.Y = c.JumpSpeed
					s.impulse = true
					endTraversal(s, cc)
					cc.Walk = launch
					if st != nil {
						refreshLaunch(st, cc, s.Direction)
					}
					s.Guard = cfg.Guard
					return
				}
				speed := s.Speed
				if s.Mode == Roll {
					// It dives forward fastest, a third of the way in while
					// stretched out in the air (see rollKeys), and slows
					// through the tuck and getting up. Over the roll it
					// averages Speed.
					u := clamp(s.Elapsed/s.Duration, 0, 1)
					speed *= 12 * u * (1 - u) * (1 - u)
				}
				sprinting := in.Run && in.Move != (rl.Vector3{})
				if s.Mode == Slide {
					// Sprinting, it slows to a run to run on at; otherwise to
					// a stop.
					floor := float32(0)
					if sprinting {
						floor = min(c.RunSpeed, s.Speed)
					}
					speed = floor + (s.Speed-floor)*max(0, 1-s.Elapsed/s.Duration)
					s.Direction = steerSlide(s.Direction, s.heading, in.Move, dt)
				}
				// Reserve the pose's leading foot/shoulder as well as capsule
				// travel, so a fast burst stops before its limbs reach a wall.
				clearance := float32(.45)
				if slideMode {
					clearance = .55
				}
				delta := rl.Vector3Scale(s.Direction, speed*dt+clearance)
				if _, hit := p.SweepCapsuleExcluding(tr.Translation, delta, cc.Radius, cc.Height, e); hit {
					endTraversal(s, cc)
				} else {
					cc.Walk = rl.Vector3Scale(s.Direction, speed)
				}
				release := float32(rollRelease)
				if slideMode {
					release = slideRelease
				}
				if s.active() && sprinting && s.Elapsed >= release*s.Duration && p.ResizeCharacter(e, cc, tr, capsuleHeight) {
					// Still sprinting: up out of the roll or slide into a
					// run, at the speed it's going, instead of standing up
					// first.
					walk := cc.Walk
					endTraversal(s, cc)
					cc.Walk = walk
				} else if s.Elapsed >= s.Duration || !s.active() {
					if p.ResizeCharacter(e, cc, tr, capsuleHeight) {
						endTraversal(s, cc)
					} else {
						s.Mode = Crouch
						cc.Walk = rl.Vector3{}
					}
				}
			case LadderClimb:
				s.Hint = "W/S climb · Space jump off"
				ladder, ok := ladders.Get(s.Ladder)
				if !ok {
					endTraversal(s, cc)
					return
				}
				if jump {
					endTraversal(s, cc)
					s.Detach = .4
					cc.Walk = rl.Vector3Scale(ladder.Facing, -cfg.KickSpeed)
					cc.Velocity.Y = cfg.KickUp
					s.impulse = true
					if st != nil {
						refreshLaunch(st, cc, rl.Vector3Negate(ladder.Facing))
					}
					return
				}
				axis := direction(rl.Vector3Subtract(ladder.Top, ladder.Bottom))
				v := -in.Move.Z * cfg.LadderSpeed
				// Ease acceleration, but releasing the key holds a rung immediately.
				if v != 0 {
					v = cc.Walk.Y + clamp(v-cc.Walk.Y, -dt*4.5, dt*4.5)
				}
				cc.Controlled = true
				cc.Walk = rl.Vector3Scale(axis, v)
				feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
				along := rl.Vector3DotProduct(rl.Vector3Subtract(feet, ladder.Bottom), axis)
				spacing := ladder.RungSpacing
				if spacing <= 0 {
					spacing = .3
				}
				s.RungSpacing, s.Phase = spacing, (along-ladderFoot)/(2*spacing)
				length := rl.Vector3Distance(ladder.Bottom, ladder.Top)
				exit := ladder.BottomExit
				// Climb off while the hands still have rail to hold: from where
				// climbing on by ladderExitRise leaves the hop to the landing.
				top := along >= max(0, length+.02-ladderExitHop-ladderExitRise) && v > 0
				leave := along <= .02 && v < 0 || top
				if top {
					exit = ladder.TopExit
				}
				if leave {
					target := rl.Vector3Add(exit, rl.Vector3{Y: cc.Height/2 + .02})
					route := Traversal{Mode: LadderExit, Start: tr.Translation, End: target, Duration: ladderExitTime, Direction: ladder.Facing, ExitTop: top, Ladder: s.Ladder}
					route.Lift = rl.Vector3{X: route.Start.X, Y: target.Y, Z: route.Start.Z}
					if !top {
						route.Duration = .35
					}
					if clearPath(p, e, cc, route.Start, route.Lift) && clearPath(p, e, cc, route.Lift, route.End) {
						*s = route
						cc.Walk, cc.Velocity = rl.Vector3{}, rl.Vector3{}
					} else {
						cc.Walk = rl.Vector3{}
						s.Hint = "Exit blocked · Space jump off"
					}
				}
			case Vault, Mantle:
				cc.Controlled = true
				next := pathPosition(s, min(1, s.Elapsed/s.Duration))
				if !clearPath(p, e, cc, tr.Translation, next) {
					endTraversal(s, cc)
					cc.Velocity = rl.Vector3{}
					return
				}
				cc.Walk = rl.Vector3Scale(rl.Vector3Subtract(next, tr.Translation), 1/dt)
				if s.Elapsed >= s.Duration {
					tr.Translation = s.End
					endTraversal(s, cc)
					cc.Velocity = rl.Vector3{}
				}
			}
			if !s.active() {
				if cc.Height < capsuleHeight && !p.ResizeCharacter(e, cc, tr, capsuleHeight) {
					s.Mode = Crouch
				}
				if st != nil {
					st.launch = horizontal(cc.Walk)
					st.aloft = false
				}
			}
			if body != nil && s.Direction != (rl.Vector3{}) {
				target := rl.QuaternionFromAxisAngle(transform.Up, float32(math.Atan2(float64(s.Direction.X), float64(s.Direction.Z))))
				body.Rotation = rl.QuaternionSlerp(body.Rotation, target, min(1, dt*10))
			}
			return
		}
		if acting {
			return
		}
		// A ladder attaches only on deliberate movement toward its face.
		if s.Detach == 0 && in.Move != (rl.Vector3{}) {
			ladders.Each(func(le ecs.Entity, l *Ladder) {
				if s.active() || rl.Vector3DotProduct(direction(in.Move), l.Facing) < .5 {
					return
				}
				feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
				axis := direction(rl.Vector3Subtract(l.Top, l.Bottom))
				along := rl.Vector3DotProduct(rl.Vector3Subtract(feet, l.Bottom), axis)
				length := rl.Vector3Distance(l.Bottom, l.Top)
				anchor := rl.Vector3Add(l.Bottom, rl.Vector3Scale(axis, clamp(along, 0, length)))
				// Stand in front of the rungs, within reach of them.
				offset := rl.Vector3Subtract(feet, anchor)
				ahead := rl.Vector3DotProduct(offset, l.Facing)
				beside := rl.Vector3Length(rl.Vector3Subtract(offset, rl.Vector3Scale(l.Facing, ahead)))
				if along < -0.2 || along > length+.2 || abs(ahead) > cfg.LadderReach || beside > max(l.Width/2, cfg.LadderReach) {
					return
				}
				target := rl.Vector3Add(anchor, rl.Vector3{Y: cc.Height/2 + .02})
				if !clearPath(p, e, cc, tr.Translation, target) {
					return
				}
				s.Start, s.End = tr.Translation, target
				s.Mode, s.Elapsed, s.Duration = LadderEnter, 0, .35
				s.Ladder = le
				s.RungSpacing = l.RungSpacing
				if s.RungSpacing <= 0 {
					s.RungSpacing = .3
				}
				s.Phase = (along - ladderFoot) / (2 * s.RungSpacing)
				s.Direction = l.Facing
				cc.Controlled = true
				cc.Walk = rl.Vector3{}
				cc.Velocity = rl.Vector3{}
				in.Jump = false
			})
			if s.active() {
				return
			}
		}
		if s.Guard == 0 && cc.Grounded && (roll || slide && in.Run && rl.Vector3Length(horizontal(cc.Velocity)) >= cfg.SlideMin) {
			if !p.ResizeCharacter(e, cc, tr, cfg.LowHeight) {
				return
			}
			s.Mode = Roll
			s.Duration = cfg.RollTime
			s.Speed = cfg.RollDistance / cfg.RollTime
			if !roll {
				s.Mode = Slide
				s.Duration = cfg.SlideTime
				s.Speed = max(cfg.SlideSpeed, rl.Vector3Length(horizontal(cc.Velocity)))
				// The way it's running, whatever it wants: it can't slide
				// off the other way.
				dir = direction(horizontal(cc.Velocity))
				s.heading = float32(math.Atan2(float64(dir.X), float64(dir.Z)))
			}
			s.Direction = dir
			s.Elapsed = 0
			s.Guard = cfg.Guard
			cc.Walk = rl.Vector3Scale(dir, s.Speed)
			if s.Mode == Roll {
				cc.Walk = rl.Vector3{}
			}
			in.Jump = false
			return
		}
		if in.Move != (rl.Vector3{}) {
			var route Traversal
			ok := false
			if !cc.Grounded {
				// In the air it can't line itself up, so heading into a
				// wall at an angle (a wall kick's, say) it climbs square on
				// to it.
				if hit, found := p.CastRayExcluding(tr.Translation, dir, cc.Radius+2*cfg.WallReach, e); found && abs(hit.Normal.Y) <= .2 {
					if into := direction(horizontal(rl.Vector3Negate(hit.Normal))); rl.Vector3DotProduct(dir, into) >= airClimbAngle {
						route, ok = obstacleRoute(p, e, cc, tr.Translation, into, cfg.WallReach)
					}
				}
			}
			if !ok {
				route, ok = obstacleRoute(p, e, cc, tr.Translation, dir, cfg.Reach)
			}
			if ok {
				s.Hint = "Space · " + route.Mode.String()
				// On the ground it turns to face the obstacle before it goes
				// over (see face), rather than spinning round in the middle
				// of the vault: asked while it faces elsewhere, it goes once
				// it's turned, if that's soon. In the air it goes at once.
				if jump {
					s.queued = obstacleWait
					in.Jump = false
				}
				facingIt := !cc.Grounded || rl.Vector3DotProduct(facing, route.Direction) >= obstacleFacing
				if s.queued > 0 && s.Guard == 0 && facingIt {
					*s = route
					s.Guard = cfg.Guard
					cc.Controlled = true
					cc.Walk = rl.Vector3{}
					cc.Velocity = rl.Vector3{}
					in.Jump = false
					return
				}
			}
		}
		// A wall kick takes the speed of a sprint's jump, or of another kick.
		if v := horizontal(cc.Walk); !cc.Grounded && (s.Bounced || rl.Vector3Length(v) >= cfg.KickMin) {
			dirs := []rl.Vector3{dir, {X: 1}, {X: -1}, {Z: 1}, {Z: -1}}
			for _, d := range dirs {
				hit, ok := p.CastRayExcluding(tr.Translation, d, cc.Radius+cfg.WallReach, e)
				if !ok || !p.StaticSurface(hit.Entity) || abs(hit.Normal.Y) > .2 || s.WallLocked && hit.Entity == s.LastWall && rl.Vector3DotProduct(hit.Normal, s.WallNormal) > .9 {
					continue
				}
				s.Hint = "Space · wall kick"
				if !jump || s.Guard > 0 {
					break
				}
				n := direction(horizontal(hit.Normal))
				// It comes off the wall like a ball: the way it came in,
				// reflected, and at least a little outwards.
				out := direction(v)
				tangent := rl.Vector3Subtract(out, rl.Vector3Scale(n, rl.Vector3DotProduct(out, n)))
				out = direction(rl.Vector3Add(tangent, rl.Vector3Scale(n, max(.35, abs(rl.Vector3DotProduct(out, n))))))
				cc.Walk = rl.Vector3Scale(out, cfg.KickSpeed)
				s.Bounced = true
				cc.Velocity.Y = cfg.KickUp
				s.impulse = true
				s.LastWall = hit.Entity
				s.WallNormal = n
				s.WallPoint = hit.Point
				s.WallLocked = true
				s.Guard = cfg.Guard
				s.Kick = wallKickTime
				// The foot nearer the wall taps it: the left one with the wall on
				// the left of the way the body is going.
				along := tangent
				if rl.Vector3LengthSqr(along) < .01 {
					along = facing
				}
				s.KickRight = rl.Vector3DotProduct(along, rl.Vector3{X: n.Z, Z: -n.X}) < 0
				in.Jump = false
				if st != nil {
					refreshLaunch(st, cc, direction(cc.Walk))
				}
				break
			}
		}
	})
}

const (
	// obstacleFacing is how squarely (the cosine of the angle off it) a
	// character has to face the way over an obstacle to start over it, and
	// obstacleWait how long it has, asked to, to turn that far.
	obstacleFacing = .94
	obstacleWait   = .8
	// airClimbAngle is how squarely (the cosine again) a character in the
	// air has to be heading into a wall to climb onto the top of it.
	airClimbAngle = .5
)

func obstacleRoute(p *physics.Physics, e ecs.Entity, cc *physics.CharacterController, center, dir rl.Vector3, reach float32) (Traversal, bool) {
	feet := center.Y - cc.Height/2
	hit, ok := p.CastRayExcluding(rl.Vector3{X: center.X, Y: feet + .3, Z: center.Z}, dir, cc.Radius+reach, e)
	if !ok || !p.StaticSurface(hit.Entity) || abs(hit.Normal.Y) > .2 {
		return Traversal{}, false
	}
	near := rl.Vector3Add(hit.Point, rl.Vector3Scale(dir, .05))
	near.Y = feet + 1.9
	top, ok := p.CastRayExcluding(near, rl.Vector3{Y: -1}, 1.9, e)
	if !ok || top.Entity != hit.Entity || top.Normal.Y < .7 {
		return Traversal{}, false
	}
	height := top.Point.Y - feet
	if height < .35 || height > 1.8 {
		return Traversal{}, false
	}
	route := Traversal{Mode: Mantle, Duration: .8, Start: center, Direction: dir}
	route.End = rl.Vector3Add(hit.Point, rl.Vector3Scale(dir, cc.Radius+.08))
	route.End.Y = top.Point.Y + cc.Height/2 + .04
	if height <= 1 {
		// Locate the far edge within the allowed depth, then require a safe landing.
		for depth := float32(.15); depth <= 1.05; depth += .1 {
			probe := rl.Vector3Add(hit.Point, rl.Vector3Scale(dir, depth))
			probe.Y = top.Point.Y + .1
			floor, found := p.CastRayExcluding(probe, rl.Vector3{Y: -1}, .25, e)
			if found && floor.Entity == hit.Entity {
				continue
			}
			landing := rl.Vector3Add(probe, rl.Vector3Scale(dir, cc.Radius+.08))
			landing.Y = top.Point.Y + .1
			ground, found := p.CastRayExcluding(landing, rl.Vector3{Y: -1}, height+.3, e)
			if found && ground.Normal.Y >= .7 {
				route.Mode = Vault
				route.Duration = 1
				route.End = rl.Vector3{X: landing.X, Y: ground.Point.Y + cc.Height/2 + .04, Z: landing.Z}
			}
			break
		}
	}
	if route.Mode == Mantle {
		probe := route.End
		probe.Y = top.Point.Y + .1
		support, found := p.CastRayExcluding(probe, rl.Vector3{Y: -1}, .2, e)
		if !found || support.Entity != hit.Entity || support.Normal.Y < .7 {
			return Traversal{}, false
		}
	}
	route.Lift = rl.Vector3{X: center.X, Y: top.Point.Y + cc.Height/2 + .08, Z: center.Z}
	across := rl.Vector3{X: route.End.X, Y: route.Lift.Y, Z: route.End.Z}
	if !clearPath(p, e, cc, center, route.Lift) || !clearPath(p, e, cc, route.Lift, across) || !clearPath(p, e, cc, across, route.End) {
		return Traversal{}, false
	}
	return route, true
}
