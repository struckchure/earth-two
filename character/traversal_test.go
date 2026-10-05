package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
	"math"
	"testing"
	"time"
)

type traversalHarness struct {
	app      *illusion.App
	root     ecs.Entity
	in       *Intent
	cc       *physics.CharacterController
	tr       *transform.Transform
	s        *Traversal
	controls *Controls
}

func newTraversalHarness(t *testing.T, feet rl.Vector3, setup func(*illusion.Commands)) *traversalHarness {
	t.Helper()
	h := &traversalHarness{controls: &Controls{Enabled: true}}
	h.app = illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}).InsertResource(illusion.R(h.controls))
	h.app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(40, 1, 40)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		cmd.Spawn(illusion.C(Default()), illusion.C(Intent{}), illusion.C(Traversal{}), illusion.C(DefaultTraversal()), illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, StepHeight: .3}), illusion.C(transform.FromTranslation(rl.Vector3Add(feet, rl.Vector3{Y: .9})))).WithChild(illusion.C(Body{}), illusion.C(State{}), illusion.C(transform.FromXYZ(0, -.9, 0)))
		if setup != nil {
			setup(cmd)
		}
	}))
	h.app.AddSystems(illusion.FixedUpdate, illusion.Chain(illusion.Fn8(traverse), illusion.Fn4(locomote)))
	t.Cleanup(h.app.Cleanup)
	h.tick(2)
	q := ecs.NewFilter1[Traversal](h.app.World).Query()
	for q.Next() {
		h.root = q.Entity()
	}
	h.in = ecs.NewMap[Intent](h.app.World).Get(h.root)
	h.cc = ecs.NewMap[physics.CharacterController](h.app.World).Get(h.root)
	h.tr = ecs.NewMap[transform.Transform](h.app.World).Get(h.root)
	h.s = ecs.NewMap[Traversal](h.app.World).Get(h.root)
	h.tick(15)
	return h
}
func (h *traversalHarness) tick(n int) {
	for range n {
		h.app.Tick(time.Second / 60)
	}
}

// faceTo turns the body to face dir, as face would in time.
func (h *traversalHarness) faceTo(dir rl.Vector3) {
	q := ecs.NewFilter1[transform.Transform](h.app.World).With(ecs.C[Body]()).Query()
	for q.Next() {
		q.Get().Rotation = rl.QuaternionFromAxisAngle(transform.Up, float32(math.Atan2(float64(dir.X), float64(dir.Z))))
	}
}
func staticBox(cmd *illusion.Commands, pos, size rl.Vector3) {
	cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(size.X, size.Y, size.Z)), illusion.C(transform.FromTranslation(pos)))
}
func TestSlideRollAndJump(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, nil)
	h.in.Slide = true
	h.tick(1)
	if h.s.Mode != Idle || h.in.Slide {
		t.Fatal("slow slide should be consumed and rejected")
	}
	h.cc.Walk = rl.Vector3{Z: 4}
	h.cc.Velocity.Z = 4
	h.in.Run = true
	h.in.Move = rl.Vector3{Z: 1}
	h.in.Slide = true
	h.tick(1)
	if h.s.Mode != Slide || h.cc.Height != .9 {
		t.Fatalf("slide mode=%v height=%v", h.s.Mode, h.cc.Height)
	}
	h.in.Jump = true
	h.tick(1)
	if h.s.active() || h.cc.Velocity.Y <= 0 || h.in.Jump {
		t.Fatal("slide jump failed")
	}
	h.in.Move = rl.Vector3{}
	h.tick(90)
	start := h.tr.Translation
	h.in.Roll = true
	h.tick(1)
	if h.s.Mode != Roll {
		t.Fatalf("roll mode=%v", h.s.Mode)
	}
	h.tick(55)
	distance := rl.Vector3Distance(horizontal(start), horizontal(h.tr.Translation))
	if distance < 1.8 || distance > 2.2 {
		t.Fatalf("roll distance=%v", distance)
	}
	h.in.Roll = true
	h.tick(1)
	if h.s.Mode != Roll {
		t.Fatal("completed roll cannot chain")
	}
}
func TestSprintingRollRunsOn(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, nil)
	h.in.Run, h.in.Move = true, rl.Vector3{Z: 1}
	h.in.Roll = true
	h.tick(1)
	if h.s.Mode != Roll {
		t.Fatalf("roll mode=%v", h.s.Mode)
	}
	steps := 0
	for h.s.Mode == Roll && steps < 60 {
		h.tick(1)
		steps++
	}
	if h.s.active() || h.cc.Height != capsuleHeight {
		t.Fatalf("sprinting roll ended in mode %v, height %v", h.s.Mode, h.cc.Height)
	}
	if limit := int(DefaultTraversal().RollTime*rollRelease*60) + 2; steps > limit {
		t.Fatalf("sprinting roll took %d steps, want it to run on after %d", steps, limit)
	}
	if v := rl.Vector3Length(horizontal(h.cc.Walk)); v < Default().WalkSpeed*2 {
		t.Fatalf("ran on out of the roll at %v, want it to keep its speed", v)
	}
	h.tick(30)
	if v := rl.Vector3Length(horizontal(h.cc.Velocity)); v < Default().RunSpeed*.9 {
		t.Fatalf("not running after the roll: %v", v)
	}
}

func TestSprintingSlideRunsOn(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, nil)
	h.cc.Walk, h.cc.Velocity.Z = rl.Vector3{Z: 4.6}, 4.6
	h.in.Run, h.in.Move = true, rl.Vector3{Z: 1}
	h.in.Slide = true
	h.tick(1)
	if h.s.Mode != Slide {
		t.Fatalf("slide mode=%v", h.s.Mode)
	}
	steps := 0
	for h.s.Mode == Slide && steps < 60 {
		h.tick(1)
		steps++
	}
	if h.s.active() || h.cc.Height != capsuleHeight {
		t.Fatalf("sprinting slide ended in mode %v, height %v", h.s.Mode, h.cc.Height)
	}
	if limit := int(DefaultTraversal().SlideTime*slideRelease*60) + 2; steps > limit {
		t.Fatalf("sprinting slide took %d steps, want it to run on after %d", steps, limit)
	}
	if v := rl.Vector3Length(horizontal(h.cc.Walk)); v < Default().RunSpeed {
		t.Fatalf("ran on out of the slide at %v, want at least running speed", v)
	}
}

func TestLowCeilingAndBlockedRoll(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Z: 1, Y: 1.2}, rl.Vector3{X: 3, Y: .2, Z: 1.4})
		staticBox(cmd, rl.Vector3{Z: 2.3, Y: 1}, rl.Vector3{X: 3, Y: 2, Z: .1})
	})
	h.in.Move = rl.Vector3{Z: 1}
	h.in.Roll = true
	h.tick(65)
	if h.tr.Translation.Z > 2 {
		t.Fatal("roll passed through wall")
	}
	if h.s.Mode != Crouch || h.cc.Height != .9 {
		t.Fatalf("stood through ceiling: %v height=%v", h.s.Mode, h.cc.Height)
	}
	h.in.Move = rl.Vector3{X: 1}
	h.tick(180)
	if h.cc.Height != 1.8 {
		t.Fatal("did not stand in open space")
	}
}
func TestVaultMantleAndBlockedRoute(t *testing.T) {
	for _, height := range []float32{.75, 1.6} {
		t.Run(map[bool]Anim{true: Vault, false: Mantle}[height < 1].String(), func(t *testing.T) {
			depth := float32(.5)
			if height > 1 {
				depth = 2
			}
			h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
				staticBox(cmd, rl.Vector3{Y: height / 2, Z: -.5 - depth/2}, rl.Vector3{X: 3, Y: height, Z: depth})
			})
			h.faceTo(rl.Vector3{Z: -1})
			h.in.Move = rl.Vector3{Z: -1}
			h.in.Jump = true
			h.tick(1)
			want := Mantle
			if height < 1 {
				want = Vault
			}
			if h.s.Mode != want {
				t.Fatalf("mode=%v want %v hint=%q", h.s.Mode, want, h.s.Hint)
			}
			h.in.Move = rl.Vector3{}
			h.tick(75)
			if h.s.active() {
				t.Fatal("route did not complete")
			}
			if h.tr.Translation.Z > -.5 {
				t.Fatal("route did not cross obstacle")
			}
			if height > 1 && h.tr.Translation.Y < height+.85 {
				t.Fatal("mantle did not land on top")
			}
		})
	}
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: .375, Z: -.75}, rl.Vector3{X: 3, Y: .75, Z: .5})
		staticBox(cmd, rl.Vector3{Y: 2.1, Z: -.8}, rl.Vector3{X: 3, Y: .2, Z: 2})
	})
	h.in.Move = rl.Vector3{Z: -1}
	h.in.Jump = true
	h.tick(1)
	if h.s.Mode == Vault || h.s.Mode == Mantle {
		t.Fatal("accepted route through ceiling")
	}
}
func TestLadderClimbHoldDetachAndPause(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(Ladder{Bottom: rl.Vector3{Z: -.2}, Top: rl.Vector3{Y: 5, Z: -.2}, Facing: rl.Vector3{Z: -1}, BottomExit: rl.Vector3{Z: .4}, TopExit: rl.Vector3{Y: 5, Z: -.2}, Width: 1}))
	})
	h.in.Move = rl.Vector3{Z: -1}
	h.tick(1)
	if h.s.Mode != LadderEnter {
		t.Fatal("ladder did not attach")
	}
	h.tick(50)
	y := h.tr.Translation.Y
	h.in.Move = rl.Vector3{}
	h.tick(20)
	if abs(h.tr.Translation.Y-y) > .01 {
		t.Fatal("ladder hold drifted")
	}
	h.controls.Enabled = false
	ecs.GetResource[physics.Settings](h.app.World).Paused = true
	elapsed := h.s.Elapsed
	h.tick(20)
	if h.s.Elapsed != elapsed || abs(h.tr.Translation.Y-y) > .01 {
		t.Fatal("pause advanced traversal")
	}
	h.controls.Enabled = true
	ecs.GetResource[physics.Settings](h.app.World).Paused = false
	h.in.Jump = true
	h.tick(1)
	if h.s.active() || h.cc.Controlled || h.cc.Velocity.Y <= 0 || h.s.Detach <= 0 {
		t.Fatal("ladder jump failed")
	}
}
func TestWallKickGuardsAndLaunch(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{Y: 1}, func(cmd *illusion.Commands) { staticBox(cmd, rl.Vector3{X: .65, Y: 2}, rl.Vector3{X: .1, Y: 4, Z: 4}) })
	h.cc.Grounded = false
	h.in.Move = rl.Vector3{X: 1}
	h.in.Jump = true
	h.cc.Walk = rl.Vector3{X: 2}
	h.tick(1)
	if h.s.Kick != 0 {
		t.Fatal("kicked off a wall without a sprint")
	}
	h.in.Jump = true
	h.cc.Walk = rl.Vector3{X: 4, Z: 3}
	h.tick(1)
	// Like a ball: out the way it came in, reflected, and it can't steer.
	if h.s.Kick == 0 || h.cc.Velocity.Y <= 0 || abs(h.cc.Walk.X+.8*3.2) > .01 || abs(h.cc.Walk.Z-.6*3.2) > .01 {
		t.Fatalf("kick failed: %+v %+v", h.s, h.cc)
	}
	h.in.Move = rl.Vector3{Z: -1}
	h.tick(5)
	if abs(h.cc.Walk.X+.8*3.2) > .01 || abs(h.cc.Walk.Z-.6*3.2) > .01 {
		t.Fatalf("steered after a wall kick: %+v", h.cc.Walk)
	}
	h.in.Move = rl.Vector3{X: 1}
	if h.s.Kick == 0 || h.cc.Walk.X >= 0 || h.cc.Velocity.Y <= 0 {
		t.Fatalf("kick failed: %+v %+v", h.s, h.cc)
	}
	speed := h.cc.Walk.X
	h.in.Jump = true
	h.tick(1)
	if h.cc.Walk.X < speed-.1 {
		t.Fatal("repeated kick bypassed guard")
	}
	if h.in.Jump {
		t.Fatal("airborne jump request retained")
	}
}
func TestWallKickUsesTheFootNearerTheWall(t *testing.T) {
	for _, along := range []float32{4, -4} {
		h := newTraversalHarness(t, rl.Vector3{Y: 1}, func(cmd *illusion.Commands) { staticBox(cmd, rl.Vector3{X: .65, Y: 2}, rl.Vector3{X: .1, Y: 4, Z: 4}) })
		h.cc.Grounded = false
		h.cc.Walk = rl.Vector3{X: 3, Z: along}
		h.in.Move = rl.Vector3{X: 1}
		h.in.Jump = true
		h.tick(1)
		// Going +Z, a wall at +X is on the left.
		if h.s.Kick == 0 || h.s.KickRight != (along < 0) {
			t.Fatalf("going %v along the wall: %+v", along, h.s)
		}
	}
}
func TestTraversalAnimationsAreNotStationaryActions(t *testing.T) {
	for _, a := range []Anim{Slide, Roll, LadderClimb, Vault, Mantle, WallKick, WallKickRight, Crouch} {
		if a.OneShot() {
			t.Fatalf("%v incorrectly locks stationary action", a)
		}
	}
}

func TestThinVaultBarrier(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: .375, Z: -.6}, rl.Vector3{X: 3, Y: .75, Z: .15})
	})
	h.faceTo(rl.Vector3{Z: -1})
	h.in.Move = rl.Vector3{Z: -1}
	h.in.Jump = true
	h.tick(1)
	if h.s.Mode != Vault {
		t.Fatalf("thin barrier mode=%v", h.s.Mode)
	}
}
func TestLadderAutomaticExits(t *testing.T) {
	for _, top := range []bool{false, true} {
		t.Run(map[bool]string{true: "top", false: "bottom"}[top], func(t *testing.T) {
			h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
				staticBox(cmd, rl.Vector3{Y: .5, Z: -1.5}, rl.Vector3{X: 3, Y: 1, Z: 1})
				cmd.Spawn(illusion.C(Ladder{Bottom: rl.Vector3{Z: -.2}, Top: rl.Vector3{Y: 1.04, Z: -.2}, Facing: rl.Vector3{Z: -1}, BottomExit: rl.Vector3{Z: .4}, TopExit: rl.Vector3{Y: 1.04, Z: -1.5}, Width: 1}))
			})
			h.in.Move = rl.Vector3{Z: -1}
			h.tick(24)
			if !top {
				h.in.Move.Z = 1
			}
			enteredExit := false
			for i := 0; i < 220; i++ {
				before := h.tr.Translation
				h.tick(1)
				if h.s.Mode == LadderExit {
					enteredExit = true
				}
				if rl.Vector3Distance(before, h.tr.Translation) > .06 {
					t.Fatal("ladder exit teleported")
				}
				if enteredExit && !h.s.active() {
					break
				}
			}
			if !enteredExit {
				t.Fatal("missing ladder exit phase")
			}
			if h.cc.Velocity.Y > .01 {
				t.Fatalf("exit launched upward: %v", h.cc.Velocity.Y)
			}
			if h.s.active() || h.cc.Controlled || h.s.Detach <= 0 {
				t.Fatalf("did not exit ladder: %+v", h.s)
			}
			if top && h.tr.Translation.Y < 1.8 {
				t.Fatalf("top exit too low %v", h.tr.Translation)
			}
		})
	}
}
func TestWallKicksCanChainBetweenWalls(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{Y: 1}, func(cmd *illusion.Commands) {
		for _, x := range []float32{-.65, .65} {
			staticBox(cmd, rl.Vector3{X: x, Y: 2}, rl.Vector3{X: .1, Y: 4, Z: 4})
		}
	})
	h.in.Move = rl.Vector3{X: 1}
	h.in.Jump = true
	h.cc.Walk = rl.Vector3{X: 4.6}
	h.tick(1)
	first := h.s.LastWall
	h.in.Move = rl.Vector3{X: -1}
	h.tick(11)
	h.in.Jump = true
	h.tick(1)
	if h.s.LastWall == first || h.cc.Walk.X <= 0 {
		t.Fatalf("second wall kick did not chain: cc=%+v position=%+v state=%+v first=%v", h.cc, h.tr.Translation, h.s, first)
	}
}

func TestSlideGetsUpByItself(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, nil)
	h.cc.Velocity.Z = 4
	h.in.Move, h.in.Run, h.in.Slide = rl.Vector3{Z: 1}, true, true
	h.tick(1)
	h.in.Move = rl.Vector3{}
	h.tick(40)
	if h.s.Mode != Slide {
		t.Fatalf("slide ended early: %v", h.s.Mode)
	}
	h.tick(10)
	// Its clip gets up out of it: no stand-up after.
	if h.s.active() || h.cc.Height != capsuleHeight {
		t.Fatalf("slide did not end standing: %v, height %v", h.s.Mode, h.cc.Height)
	}
}

func TestLadderPhaseFollowsDistanceAndHolds(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(Ladder{Bottom: rl.Vector3{Z: -.2}, Top: rl.Vector3{Y: 5, Z: -.2}, Facing: rl.Vector3{Z: -1}, Width: 1, RungSpacing: .25}))
	})
	h.in.Move.Z = -1
	h.tick(80)
	if h.s.Mode != LadderClimb {
		t.Fatalf("not climbing: %v", h.s.Mode)
	}
	want := (h.tr.Translation.Y - h.cc.Height/2 - h.cc.Walk.Y/60 - ladderFoot) / .5
	if abs(h.s.Phase-want) > .005 {
		t.Fatalf("phase=%v distance phase=%v", h.s.Phase, want)
	}
	h.in.Move = rl.Vector3{}
	h.tick(1)
	phase := h.s.Phase
	h.tick(20)
	if abs(h.s.Phase-phase) > .001 {
		t.Fatal("hold advanced contact cycle")
	}
	h.in.Move.Z = 1
	h.tick(15)
	if h.s.Phase >= phase {
		t.Fatal("descending did not reverse contact cycle")
	}
}

func TestLadderBlockedExitHoldsWithoutLaunching(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(Ladder{Bottom: rl.Vector3{Z: -.2}, Top: rl.Vector3{Y: 2.84, Z: -.2}, Facing: rl.Vector3{Z: -1}, TopExit: rl.Vector3{Y: 2.84, Z: -1.5}, Width: 1}))
		staticBox(cmd, rl.Vector3{Y: 4, Z: -1.5}, rl.Vector3{X: 2, Y: 1, Z: 1})
	})
	h.in.Move.Z = -1
	h.tick(260)
	if h.s.Mode != LadderClimb || h.s.Hint != "Exit blocked · Space jump off" {
		t.Fatalf("blocked exit state: %+v", h.s)
	}
	y := h.tr.Translation.Y
	h.tick(30)
	if abs(h.tr.Translation.Y-y) > .01 || abs(h.cc.Velocity.Y) > .01 {
		t.Fatal("blocked ladder exit drifted")
	}
	h.in.Jump = true
	h.tick(1)
	if h.s.active() || h.cc.Velocity.Y <= 0 {
		t.Fatal("blocked exit trapped character")
	}
}

func TestSlideStopsBeforeItsLeadingFootReachesWall(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) { staticBox(cmd, rl.Vector3{Y: 1, Z: 2}, rl.Vector3{X: 4, Y: 2, Z: .2}) })
	h.cc.Walk = rl.Vector3{Z: 5}
	h.cc.Velocity.Z = 5
	h.in.Move, h.in.Run, h.in.Slide = rl.Vector3{Z: 1}, true, true
	h.tick(1)
	for range 70 {
		h.tick(1)
		if h.s.Mode != Slide {
			break
		}
	}
	if h.s.Mode == Slide {
		t.Fatal("slide never stopped")
	}
	if h.tr.Translation.Z > .3+1 {
		t.Fatalf("capsule stopped too close for leading foot: %v", h.tr.Translation.Z)
	}
}
func TestSlideRecoveryAllowsMovementJumpAndRoll(t *testing.T) {
	for _, next := range []string{"walk", "jump", "roll"} {
		t.Run(next, func(t *testing.T) {
			h := newTraversalHarness(t, rl.Vector3{}, nil)
			standUp(h.s, h.cc)
			h.in.Move = rl.Vector3{Z: 1}
			if next == "jump" {
				h.in.Jump = true
			}
			if next == "roll" {
				h.in.Roll = true
			}
			start := h.tr.Translation
			h.tick(1)
			if next == "jump" && (h.cc.Velocity.Y <= 0 || h.s.active()) {
				t.Fatal("recovery swallowed jump")
			}
			if next == "roll" && h.s.Mode != Roll {
				t.Fatal("recovery swallowed chained roll")
			}
			if next == "walk" {
				h.tick(16)
				if h.tr.Translation.Z <= start.Z+.05 {
					t.Fatal("recovery locked movement")
				}
			}
		})
	}
}

func TestObstaclesAndWallsNeedTheBodyNear(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: .375, Z: -1.15}, rl.Vector3{X: 3, Y: .75, Z: .5})
	})
	h.in.Move = rl.Vector3{Z: -1}
	h.in.Jump = true
	h.tick(1)
	if h.s.Mode == Vault || h.s.Hint != "" {
		t.Fatalf("vaulted from out of reach: %v hint=%q", h.s.Mode, h.s.Hint)
	}
	h = newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(Ladder{Bottom: rl.Vector3{Z: -.4}, Top: rl.Vector3{Y: 5, Z: -.4}, Facing: rl.Vector3{Z: -1}, Width: 1}))
	})
	h.in.Move = rl.Vector3{Z: -1}
	h.tick(1)
	if h.s.active() {
		t.Fatal("ladder attached from out of reach")
	}
	h = newTraversalHarness(t, rl.Vector3{Y: 1}, func(cmd *illusion.Commands) { staticBox(cmd, rl.Vector3{X: .75, Y: 2}, rl.Vector3{X: .1, Y: 4, Z: 4}) })
	h.cc.Grounded = false
	h.in.Move = rl.Vector3{X: 1}
	h.in.Jump = true
	h.tick(1)
	if h.s.Kick != 0 {
		t.Fatal("kicked a wall out of reach")
	}
}

func TestWallKickLandingKeepsASprintGoing(t *testing.T) {
	for _, run := range []bool{false, true} {
		h := newTraversalHarness(t, rl.Vector3{}, nil)
		h.s.Bounced = true
		h.cc.Grounded = true
		h.cc.Walk = rl.Vector3{Z: 3.2} // the way the body faces
		h.in.Move, h.in.Run = rl.Vector3{Z: 1}, run
		h.tick(1)
		want := DefaultTraversal().LandDelay
		if run {
			want = 0
		}
		if h.s.Bounced || abs(h.s.Land-want) > .001 {
			t.Fatalf("run %v: landing %+v", run, h.s)
		}
		h.tick(6)
		// Walking it pulls up for the landing; sprinting it runs straight on.
		if speed := h.cc.Walk.Z; run != (speed > 3.2) {
			t.Fatalf("run %v: going %v through the landing", run, speed)
		}
	}
}

func TestVaultWaitsToFaceTheObstacle(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: .375, Z: -.75}, rl.Vector3{X: 3, Y: .75, Z: .5})
	})
	// Its back to the barrier: asked to vault, it has to turn round first.
	h.in.Move = rl.Vector3{Z: -1}
	h.in.Jump = true
	h.tick(1)
	if h.s.Mode != Idle || h.in.Jump || !h.cc.Grounded {
		t.Fatalf("facing away: mode=%v jump=%v grounded=%v, want it waiting on the ground", h.s.Mode, h.in.Jump, h.cc.Grounded)
	}
	h.faceTo(rl.Vector3{Z: -1})
	h.tick(1)
	if h.s.Mode != Vault {
		t.Fatalf("turned to face it: mode=%v, want vault", h.s.Mode)
	}

	// Asked, but still facing away when the wait runs out: it stays put.
	h = newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{Y: .375, Z: -.75}, rl.Vector3{X: 3, Y: .75, Z: .5})
	})
	h.in.Move = rl.Vector3{Z: -1}
	h.in.Jump = true
	h.tick(60)
	h.faceTo(rl.Vector3{Z: -1})
	h.tick(1)
	if h.s.Mode != Idle {
		t.Fatalf("after the wait: mode=%v, want idle", h.s.Mode)
	}
}

func TestMantleFromTheAir(t *testing.T) {
	h := newTraversalHarness(t, rl.Vector3{}, func(cmd *illusion.Commands) {
		staticBox(cmd, rl.Vector3{X: 2, Y: 2}, rl.Vector3{X: 3, Y: 4, Z: 16})
	})
	// In the air beside the block, its top 1.5 m above the feet, facing
	// along it and heading into it at an angle, as off a wall kick.
	h.tr.Translation = rl.Vector3{X: .1, Y: 2.5 + .9}
	h.cc.Grounded = false
	h.in.Move = direction(rl.Vector3{X: 1, Z: 1})
	h.in.Jump = true
	h.tick(1)
	if h.s.Mode != Mantle || h.s.Direction != (rl.Vector3{X: 1}) {
		t.Fatalf("mode=%v direction=%v hint=%q, want a mantle square on to the block", h.s.Mode, h.s.Direction, h.s.Hint)
	}
	h.tick(60)
	if h.s.active() || !h.cc.Grounded || h.tr.Translation.Y < 4.8 || h.tr.Translation.X < .5 {
		t.Fatalf("after the mantle: mode=%v grounded=%v at %v, want standing on the block", h.s.Mode, h.cc.Grounded, h.tr.Translation)
	}
}
