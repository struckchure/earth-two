package world_test

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// block is the Hull test block's colliders and ladders with a character in
// it, run through physics and traversal without drawing anything.
type block struct {
	app  *illusion.App
	in   *character.Intent
	cc   *physics.CharacterController
	tr   *transform.Transform
	s    *character.Traversal
	body *transform.Transform
}

func stub[T any](string) (T, error) { var v T; return v, nil }

func newBlock(t *testing.T, feet rl.Vector3, facing rl.Vector3) *block {
	t.Helper()
	b := &block{}
	b.app = illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	// No window: the models load as nothing.
	asset.RegisterLoader(b.app, stub[render.Model], nil)
	asset.RegisterLoader(b.app, stub[render.Animations], nil)
	asset.RegisterLoader(b.app, stub[render.Texture], nil)
	// And no keyboard: the test is the player.
	b.app.InsertResource(
		illusion.R(&asset.Settings{Root: "../assets"}),
		illusion.R(input.NewButtonInput[input.Key]()),
		illusion.R(input.NewButtonInput[input.MouseButton]()),
		illusion.R(&input.Mouse{}),
	)
	b.app.AddPlugins(character.Plugin{}, world.Plugin{Manifest: "world/world.json"})
	b.app.AddSystems(illusion.Startup, illusion.Fn2(func(cmd *illusion.Commands, kit *illusion.Res[world.Kit]) {
		// The ground under the deck, as the game has it.
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(40, 1, 40)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		if err := kit.Get().SpawnLayout(cmd, "world/hull_block.json"); err != nil {
			t.Error(err)
		}
		cmd.Spawn(
			illusion.C(character.Default()), illusion.C(character.Intent{}), illusion.C(character.Traversal{}), illusion.C(character.DefaultTraversal()),
			illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, StepHeight: .3}),
			illusion.C(transform.FromTranslation(rl.Vector3Add(feet, rl.Vector3{Y: .9}))),
		).WithChild(illusion.C(character.Body{}), illusion.C(character.State{}), illusion.C(transform.FromXYZ(0, -.9, 0)))
	}))
	t.Cleanup(b.app.Cleanup)
	b.tick(2)
	q := ecs.NewFilter1[character.Traversal](b.app.World).Query()
	var root ecs.Entity
	for q.Next() {
		root = q.Entity()
	}
	b.in = ecs.NewMap[character.Intent](b.app.World).Get(root)
	b.cc = ecs.NewMap[physics.CharacterController](b.app.World).Get(root)
	b.tr = ecs.NewMap[transform.Transform](b.app.World).Get(root)
	b.s = ecs.NewMap[character.Traversal](b.app.World).Get(root)
	bq := ecs.NewFilter1[transform.Transform](b.app.World).With(ecs.C[character.Body]()).Query()
	for bq.Next() {
		b.body = bq.Get()
	}
	b.tick(15)
	b.face(facing)
	return b
}

func (b *block) tick(n int) {
	for range n {
		b.app.Tick(time.Second / 60)
	}
}

func (b *block) face(dir rl.Vector3) {
	b.body.Rotation = rl.QuaternionFromAxisAngle(transform.Up, float32(math.Atan2(float64(dir.X), float64(dir.Z))))
}

func (b *block) feet() rl.Vector3 {
	return rl.Vector3Subtract(b.tr.Translation, rl.Vector3{Y: b.cc.Height / 2})
}

// until ticks until done says so, for at most n ticks, and says whether it did.
func (b *block) until(n int, done func() bool) bool {
	for range n {
		b.tick(1)
		if done() {
			return true
		}
	}
	return false
}

// The positions below are the layout's (tools/world/hull_block.py) in the
// game's frame: Blender's (x, y) is the game's (x, -z).

func TestBlockVaultsTheCrate(t *testing.T) {
	// The crate at (1, 1), its long side to the south.
	b := newBlock(t, rl.Vector3{X: 1, Z: 1.95}, rl.Vector3{Z: -1})
	b.in.Move = rl.Vector3{Z: -1}
	b.in.Jump = true
	// It turns to face the crate first, if it has to.
	if !b.until(45, func() bool { return b.s.Mode == character.Vault }) {
		t.Fatalf("mode %v, want a vault (hint %q)", b.s.Mode, b.s.Hint)
	}
	b.in.Move = rl.Vector3{}
	b.tick(90)
	if f := b.feet(); f.Z > .5 || f.Y > .1 {
		t.Fatalf("ended at %v, not over the crate", f)
	}
}

func TestBlockMantlesTheTallCrate(t *testing.T) {
	// The tall crate at (5, 3), from the north.
	b := newBlock(t, rl.Vector3{X: 5, Z: 2.05}, rl.Vector3{Z: 1})
	b.in.Move = rl.Vector3{Z: 1}
	b.in.Jump = true
	// It turns to face the crate first, if it has to.
	if !b.until(45, func() bool { return b.s.Mode == character.Mantle }) {
		t.Fatalf("mode %v, want a mantle (hint %q)", b.s.Mode, b.s.Hint)
	}
	b.in.Move = rl.Vector3{}
	b.tick(90)
	if f := b.feet(); f.Y < 1.55 {
		t.Fatalf("ended at %v, not on top of the crate", f)
	}
}

func TestBlockSlidesUnderTheDuct(t *testing.T) {
	// The duct across the gap at (-3, -1), from the south at a run.
	b := newBlock(t, rl.Vector3{X: -3, Z: .6}, rl.Vector3{Z: -1})
	b.in.Move = rl.Vector3{Z: -1}
	b.in.Run = true
	b.cc.Walk = rl.Vector3{Z: -6}
	b.cc.Velocity.Z = -6
	b.in.Slide = true
	b.tick(1)
	if b.s.Mode != character.Slide {
		t.Fatalf("mode %v, want a slide (hint %q)", b.s.Mode, b.s.Hint)
	}
	if !b.until(120, func() bool { return b.feet().Z < -1.6 }) {
		t.Fatalf("stuck at %v under the duct", b.feet())
	}
}

func TestBlockClimbsTheLadderToTheCatwalk(t *testing.T) {
	// The ladder at (-5, -4.37), climbed facing north.
	b := newBlock(t, rl.Vector3{X: -5, Z: -3.4}, rl.Vector3{Z: -1})
	b.in.Move = rl.Vector3{Z: -1}
	if !b.until(60, func() bool { return b.s.Mode == character.LadderEnter || b.s.Mode == character.LadderClimb }) {
		t.Fatalf("didn't get on the ladder: at %v, mode %v", b.feet(), b.s.Mode)
	}
	exited := false
	b.until(1200, func() bool {
		if b.s.Mode == character.LadderExit {
			exited = true
		}
		return exited && b.s.Mode != character.LadderExit
	})
	if f := b.feet(); !exited || f.Y < 3.5 || f.Z > -4.6 {
		t.Fatalf("ended at %v (mode %v), not on the catwalk", f, b.s.Mode)
	}
}

func TestBlockWalksUpTheStairs(t *testing.T) {
	// The stairs at x 7, rising north from z 1.35 to the catwalk.
	b := newBlock(t, rl.Vector3{X: 7, Z: 2.2}, rl.Vector3{Z: -1})
	b.in.Move = rl.Vector3{Z: -1}
	if !b.until(600, func() bool { return b.feet().Y > 3.5 && b.feet().Z < -4.8 }) {
		t.Fatalf("got to %v, not up on the catwalk", b.feet())
	}
}

func TestBlockWallKicksUpTheCorridor(t *testing.T) {
	// The corridor's walls face each other at x 8.15 and 9.85.
	// Dropped in a metre up, it's still in the air, as off a jump; level
	// with the seam between two walls, which mustn't let its probe through.
	b := newBlock(t, rl.Vector3{X: 9.3, Y: 1}, rl.Vector3{X: 1})
	b.in.Move = rl.Vector3{X: 1}
	b.in.Jump = true
	b.cc.Walk = rl.Vector3{X: 4.6}
	b.tick(1)
	first := b.s.LastWall
	if b.cc.Walk.X >= 0 {
		t.Fatalf("no kick off the east wall: %+v", b.s)
	}
	b.in.Move = rl.Vector3{X: -1}
	b.tick(11)
	b.in.Jump = true
	b.tick(1)
	if b.s.LastWall == first || b.cc.Walk.X <= 0 {
		t.Fatalf("no second kick off the west wall: at %v, %+v", b.feet(), b.s)
	}
}
