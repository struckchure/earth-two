package game

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

// landfall is Landfall's colliders and ladders, on the game's ground, with
// a character dropped in at feet: run through physics and traversal
// without drawing anything.
type landfall struct {
	app *illusion.App
	in  *character.Intent
	cc  *physics.CharacterController
	tr  *transform.Transform
}

func stub[T any](string) (T, error) { var v T; return v, nil }

func newLandfall(t *testing.T, feet rl.Vector3) *landfall {
	t.Helper()
	l := &landfall{app: illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})}
	// No window: the models load as nothing. No keyboard: the test moves.
	asset.RegisterLoader(l.app, stub[render.Model], nil)
	asset.RegisterLoader(l.app, stub[render.Animations], nil)
	asset.RegisterLoader(l.app, stub[render.Texture], nil)
	l.app.InsertResource(
		illusion.R(&asset.Settings{Root: "../assets"}),
		illusion.R(input.NewButtonInput[input.Key]()),
		illusion.R(input.NewButtonInput[input.MouseButton]()),
		illusion.R(&input.Mouse{}),
	)
	l.app.AddPlugins(character.Plugin{}, world.Plugin{Manifest: "world/world.json"})
	l.app.AddSystems(illusion.Startup, illusion.Fn2(func(cmd *illusion.Commands, kit *illusion.Res[world.Kit]) {
		// The ground and the pieces on it, as setup has them.
		cmd.Spawn(illusion.C(transform.Identity()), illusion.C(physics.Static), illusion.C(terrainCollider()))
		placed, err := world.Layout("../assets", "world/landfall.json")
		if err != nil {
			t.Fatal(err)
		}
		if err := spawnOnTerrain(cmd, kit.Get(), placed); err != nil {
			t.Error(err)
		}
		cmd.Spawn(
			illusion.C(character.Default()), illusion.C(character.Intent{}), illusion.C(character.Traversal{}), illusion.C(character.DefaultTraversal()),
			illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, StepHeight: .3}),
			illusion.C(transform.FromTranslation(rl.Vector3Add(feet, rl.Vector3{Y: .9}))),
		).WithChild(illusion.C(character.Body{}), illusion.C(character.State{}), illusion.C(transform.FromXYZ(0, -.9, 0)))
	}))
	t.Cleanup(l.app.Cleanup)
	l.tick(2)
	q := ecs.NewFilter1[character.Traversal](l.app.World).Query()
	var root ecs.Entity
	for q.Next() {
		root = q.Entity()
	}
	l.in = ecs.NewMap[character.Intent](l.app.World).Get(root)
	l.cc = ecs.NewMap[physics.CharacterController](l.app.World).Get(root)
	l.tr = ecs.NewMap[transform.Transform](l.app.World).Get(root)
	return l
}

func (l *landfall) tick(n int) {
	for range n {
		l.app.Tick(time.Second / 60)
	}
}

func (l *landfall) feet() rl.Vector3 {
	return rl.Vector3Subtract(l.tr.Translation, rl.Vector3{Y: l.cc.Height / 2})
}

// The positions below are tools/world/landfall.py's in the game's frame:
// Blender's (x, y) is the game's (x, -z).

func TestArrivalStandsOnTheGround(t *testing.T) {
	l := newLandfall(t, rl.Vector3Add(arrival, rl.Vector3{Y: .5}))
	l.tick(60)
	if f := l.feet(); !l.cc.Grounded || f.Y < -.1 || f.Y > .1 {
		t.Fatalf("arriving, feet at %v (grounded %v), want on the ground", f, l.cc.Grounded)
	}
	// And free to walk off towards the Hull.
	start := l.tr.Translation
	l.in.Move = rl.Vector3{X: -1}
	l.tick(60)
	if moved := start.X - l.tr.Translation.X; moved < 1 {
		t.Fatalf("walked %v m from the arrival, want free to walk", moved)
	}
}

func TestTheStacksAreADeckUp(t *testing.T) {
	// Dropped onto the Stacks, over the market's north side.
	l := newLandfall(t, rl.Vector3{X: -15, Y: 5, Z: -5})
	l.tick(90)
	if f := l.feet(); !l.cc.Grounded || f.Y < 3.4 || f.Y > 3.8 {
		t.Fatalf("on the Stacks, feet at %v (grounded %v), want on its deck at 3.6", f, l.cc.Grounded)
	}
}

func TestTheRoadRunsThroughTheSouthGate(t *testing.T) {
	// Outside the gate on the road, walking north through it, off the
	// middle of the way its open doors leave (x -2.25 to 2.25).
	l := newLandfall(t, rl.Vector3{X: -.7, Z: 48})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	l.tick(6 * 60) // at a walk, 1.6 m/s
	if z := l.tr.Translation.Z; z > 40 {
		t.Fatalf("walking north from outside, stuck at z %v, want in through the gate (z < 44)", z)
	}
}

func TestCharterRowsGateIsOpen(t *testing.T) {
	// South of Charter Row's gate on the road from the Hull, walking north.
	l := newLandfall(t, rl.Vector3{X: -1, Z: -16})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	l.tick(5 * 60)
	if z := l.tr.Translation.Z; z > -22 {
		t.Fatalf("walking north to Charter Row, stuck at z %v, want in through its gate (z < -20)", z)
	}
}

func TestNoWayRoundTheSouthGate(t *testing.T) {
	// Either side of the gate, where the dome line meets it, walking north.
	for _, x := range []float32{-4.7, 4.5} {
		l := newLandfall(t, rl.Vector3{X: x, Z: 48})
		l.tick(10)
		l.in.Move = rl.Vector3{Z: -1}
		l.tick(5 * 60)
		if z := l.tr.Translation.Z; z < 43 {
			t.Errorf("walking north at x %v, got in to z %v round the gate", x, z)
		}
	}
}

func TestTheStacksStairsCountAsStairs(t *testing.T) {
	// At the foot of the stairs up to the Stacks (x -5, rising north from
	// z 6), walking up them.
	l := newLandfall(t, rl.Vector3{X: -5, Z: 7})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	on := false
	for range 6 * 60 {
		l.tick(1)
		// What character's animation takes for stairs: ground sloping
		// 18° to 50°, and going against its lean (uphill). The controller
		// gives no vertical speed on the ground, so not that.
		n, v := l.cc.GroundNormal, l.cc.Velocity
		slope := math.Acos(float64(n.Y)) * 180 / math.Pi
		if l.cc.Grounded && slope > 18 && slope < 50 && v.X*n.X+v.Z*n.Z < 0 {
			on = true
			break
		}
	}
	if !on {
		t.Fatalf("walking up the stairs, never on them: at %v, ground %v", l.feet(), l.cc.GroundNormal)
	}
}
