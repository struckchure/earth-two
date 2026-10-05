package game

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

func TestOrbitStartsBehind(t *testing.T) {
	o := newOrbit()
	sh := o.shot()
	if sh.offset.Z <= 0 || sh.offset.Y <= sh.look.Y || math.Abs(float64(sh.offset.X)) > 1e-5 {
		t.Errorf("camera starts at %v, want behind and above a player facing -Z", sh.offset)
	}
	if f := o.forward(); rl.Vector3Distance(f, rl.Vector3{Z: -1}) > 1e-5 {
		t.Errorf("camera starts facing %v, want -Z", f)
	}
}

func TestOrbitTurnsAndTilts(t *testing.T) {
	o := newOrbit()
	o.turn(rl.Vector2{X: 100})
	if f := o.forward(); f.X <= 0 {
		t.Errorf("moving the mouse right faces %v, want turned right (+X)", f)
	}
	o.turn(rl.Vector2{Y: 1e4})
	if o.pitch != orbitHigh {
		t.Errorf("tilted to %v, want held at %v", o.pitch, orbitHigh)
	}
	o.turn(rl.Vector2{Y: -1e4})
	if o.pitch != orbitLow {
		t.Errorf("tilted to %v, want held at %v", o.pitch, orbitLow)
	}
}

func TestOrbitRecenters(t *testing.T) {
	facingX := float32(math.Pi / 2) // the body faces +X: behind it is -X
	step := func(o *orbit, moving bool, secs float32) {
		for range int(secs * 60) {
			o.recenter(facingX, moving, 1./60)
		}
	}

	o := newOrbit()
	o.turn(rl.Vector2{X: 1}) // just touched the mouse
	step(o, true, recenterDelay/2)
	if math.Abs(float64(o.yaw)) > .01 {
		t.Errorf("followed round %v while the mouse was in use", o.yaw)
	}
	step(o, true, 3)
	if d := math.Abs(float64(wrapYaw(o.yaw - (-math.Pi / 2)))); d > .05 {
		t.Errorf("after moving a while it's at %v, want behind the body (%v)", o.yaw, -math.Pi/2)
	}

	still := newOrbit()
	step(still, false, 3)
	if still.yaw != 0 {
		t.Errorf("standing still, it followed round to %v", still.yaw)
	}

	// Heading back at the camera, it doesn't follow round.
	back := newOrbit()
	for range 180 {
		back.recenter(0, true, 1./60) // facing +Z, at the camera
	}
	if back.yaw != 0 {
		t.Errorf("running at the camera, it swung round to %v", back.yaw)
	}
}

// withPhysics runs check against a world of boxes, each a centre and a size,
// with an entity standing in for the player.
func withPhysics(t *testing.T, boxes [][2]rl.Vector3, check func(p *physics.Physics, player ecs.Entity)) {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	t.Cleanup(app.Cleanup)
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		for _, b := range boxes {
			cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(b[1].X, b[1].Y, b[1].Z)), illusion.C(transform.FromTranslation(b[0])))
		}
	}))
	app.Tick(time.Second / 60)
	player := app.World.NewEntity()
	ran := false
	app.AddSystems(illusion.Update, illusion.Fn1(func(p *physics.Physics) {
		if !ran {
			ran = true
			check(p, player)
		}
	}))
	app.Tick(time.Second / 60)
	if !ran {
		t.Fatal("check didn't run")
	}
}

func TestSpringArmPullsIn(t *testing.T) {
	pivot, eye := rl.Vector3{Y: 1.6}, rl.Vector3{Y: 2.8, Z: 4.4}
	// A wall across the arm, its near face 2 m behind the player.
	withPhysics(t, [][2]rl.Vector3{{{Z: 2.5}, {X: 10, Y: 10, Z: 1}}}, func(p *physics.Physics, player ecs.Entity) {
		got := springArm(p, pivot, eye, player)
		if got.Z > 2-armRadius+1e-3 || got.Z < 1.5 {
			t.Errorf("camera at %v, want pulled in just short of the wall at z 2", got)
		}
		if d := rl.Vector3Distance(rl.Vector3Normalize(rl.Vector3Subtract(got, pivot)), rl.Vector3Normalize(rl.Vector3Subtract(eye, pivot))); d > 1e-3 {
			t.Errorf("camera at %v, off the arm from %v to %v", got, pivot, eye)
		}
	})
	withPhysics(t, nil, func(p *physics.Physics, player ecs.Entity) {
		if got := springArm(p, pivot, eye, player); rl.Vector3Distance(got, eye) > 1e-5 {
			t.Errorf("camera with nothing in the way at %v, want %v", got, eye)
		}
	})
}

func TestLookComesOutFromUnderALowCeiling(t *testing.T) {
	// The duct's underside 1.075 m up, over a crouched capsule centred
	// 0.45 m up.
	withPhysics(t, [][2]rl.Vector3{{{Y: 1.375}, {X: 4, Y: .6, Z: 4}}}, func(p *physics.Physics, player ecs.Entity) {
		centre, look := rl.Vector3{Y: .45}, rl.Vector3{Y: 1.15}
		got := clearAbove(p, centre, look, player)
		if got.Y > 1.075-armRadius+1e-3 || got.Y <= centre.Y {
			t.Errorf("look at %v, want brought down under the duct at 1.075", got)
		}
	})
}
