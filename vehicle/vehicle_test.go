package vehicle

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func stub[T any](string) (T, error) { var v T; return v, nil }

// testBuggy is a buggy-sized four-wheel-drive car.
func testBuggy() *Spec {
	s := &Spec{
		Handling: "buggy",
		Chassis:  []Box{{Center: [3]float32{0, 0.95, 0}, Size: [3]float32{1.6, 0.7, 3.6}}},
		Seats: []SeatSpec{{At: [3]float32{0.35, 0.55, 0}, Pose: "drive",
			Exits: [][3]float32{{1.8, 0, 0}, {-1.8, 0, 0}, {0, 0, -3}}}},
	}
	for _, z := range []float32{1.3, -1.3} {
		for _, x := range []float32{0.85, -0.85} {
			s.Wheels = append(s.Wheels, WheelSpec{Piece: "buggy_wheel", At: [3]float32{x, 0.38, z}, Radius: 0.38,
				Width: 0.28, Left: x > 0, Steer: z > 0, Drive: true, HandBrake: z < 0})
		}
	}
	return s
}

type rig struct {
	app         *illusion.App
	car, player ecs.Entity
	keys        *input.Keys
	prompt      *Prompt
	driving     *Driving
}

// newRig is a buggy parked on a floor 400 m across, and the player standing
// beside its driving seat, without a window.
func newRig(t *testing.T) *rig { return newRigWith(t, "buggy", testBuggy()) }

// newRigWith parks name, built as spec, with the player beside its seat.
func newRigWith(t *testing.T, name string, spec *Spec) *rig {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	asset.RegisterLoader(app, stub[render.Model], nil)
	asset.RegisterLoader(app, stub[render.Animations], nil)
	asset.RegisterLoader(app, stub[render.Texture], nil)
	keys := input.NewButtonInput[input.Key]()
	app.InsertResource(
		illusion.R(&asset.Settings{Root: "../assets"}),
		illusion.R(keys),
		illusion.R(input.NewButtonInput[input.MouseButton]()),
		illusion.R(&input.Mouse{}),
	)
	app.AddPlugins(character.Plugin{Models: []character.Model{{Path: "characters/man.glb", Scale: 1,
		Clips: map[character.Anim]character.Clip{character.Idle: {Name: "idle"}, character.Drive: {Name: "drive"}}}}},
		Plugin{})
	app.AddSystems(illusion.Startup, illusion.Fn2(func(cmd *illusion.Commands, r *illusion.Res[character.Roster]) {
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(400, 1, 400)),
			illusion.C(physics.Material{Friction: 0.9}), illusion.C(transform.FromXYZ(0, -.5, 0)))
		none := func(asset.Handle[render.Model]) []illusion.Component { return nil }
		wheel := func(string) asset.Handle[render.Model] { return asset.Handle[render.Model]{} }
		if _, err := Spawn(cmd, name, spec, asset.Handle[render.Model]{}, wheel, none,
			rl.Vector3{}, rl.QuaternionIdentity()); err != nil {
			t.Fatal(err)
		}
		seat := spec.Seats[0].At
		r.Get().Spawn(cmd, 0, rl.Vector3{X: seat[0] + 1.6, Z: seat[2]}, illusion.C(character.Player{}))
	}))
	t.Cleanup(app.Cleanup)
	g := &rig{app: app, keys: keys}
	g.tick(30)
	g.car = single[Drivable](app)
	g.player = single[character.Player](app)
	g.prompt = ecs.GetResource[Prompt](app.World)
	g.driving = ecs.GetResource[Driving](app.World)
	return g
}

func single[T any](app *illusion.App) ecs.Entity {
	var e ecs.Entity
	for q := ecs.NewFilter1[T](app.World).Query(); q.Next(); {
		e = q.Entity()
	}
	return e
}

func get[T any](g *rig, e ecs.Entity) *T { return ecs.NewMap[T](g.app.World).Get(e) }

func (g *rig) tick(n int) {
	for range n {
		g.app.Tick(time.Second / 60)
	}
}

// tap presses and lets go of key over a frame.
func (g *rig) tap(key input.Key) {
	g.keys.Press(key)
	g.tick(1)
	g.keys.Release(key)
	g.keys.Clear()
	g.tick(1)
}

func TestGetInDriveGetOutAndPark(t *testing.T) {
	g := newRig(t)
	if *get[physics.RigidBody](g, g.car) != physics.Static {
		t.Fatal("a parked vehicle should be Static")
	}
	if g.prompt.Key != "E" || g.prompt.Text != "Drive the buggy" {
		t.Fatalf("beside the seat, the player should be offered the buggy, got %+v", *g.prompt)
	}

	g.tap(rl.KeyE)
	if !ecs.NewMap[character.Seated](g.app.World).Has(g.player) {
		t.Fatal("E should seat the player")
	}
	if ecs.NewMap[physics.CharacterController](g.app.World).Has(g.player) {
		t.Fatal("seated, the player shouldn't keep a capsule to bump the buggy with")
	}
	if *get[physics.RigidBody](g, g.car) != physics.Dynamic || !g.driving.Active() || g.driving.Name != "buggy" {
		t.Fatal("getting in should wake the buggy, and say the player's driving it")
	}
	if act := get[character.Intent](g, g.player).Act; act != character.Idle {
		t.Fatalf("getting in shouldn't also interact, got %v", act)
	}
	g.tick(60)
	if st := get[physics.VehicleState](g, g.car); st.Touching != 4 {
		t.Fatalf("it should stand on its 4 wheels, %d touch", st.Touching)
	}
	rest := get[transform.Transform](g, g.car).Translation
	if math.Abs(float64(rest.Y)) > 0.03 {
		t.Fatalf("woken, it should settle where it was parked (its suspension at rest), not at y=%v", rest.Y)
	}

	g.keys.Press(rl.KeyW)
	g.tick(180)
	g.keys.Release(rl.KeyW)
	car := get[transform.Transform](g, g.car).Translation
	if car.Z < 8 {
		t.Fatalf("W should drive it forward, it's at %v", car)
	}
	seat := get[transform.Transform](g, g.player).Translation
	if rl.Vector3Distance(seat, car) > 2 {
		t.Fatalf("the driver should go with it: driver at %v, buggy at %v", seat, car)
	}

	// Too fast to get out.
	g.tap(rl.KeyE)
	if g.prompt.Noting() != "Slow down to get out" || !ecs.NewMap[character.Seated](g.app.World).Has(g.player) {
		t.Fatalf("at speed, E shouldn't let the player out: note %q, seated %v, speed %v", g.prompt.Noting(),
			ecs.NewMap[character.Seated](g.app.World).Has(g.player), get[physics.VehicleState](g, g.car).Speed)
	}

	g.keys.Press(rl.KeyS)
	for range 600 {
		g.tick(1)
		if math.Abs(float64(get[physics.VehicleState](g, g.car).Speed)) < 0.5 {
			break
		}
	}
	g.keys.Release(rl.KeyS)
	if s := get[physics.VehicleState](g, g.car).Speed; math.Abs(float64(s)) > 0.5 {
		t.Fatalf("S should brake it to a stop, still at %v m/s", s)
	}

	g.tap(rl.KeyE)
	if ecs.NewMap[character.Seated](g.app.World).Has(g.player) {
		t.Fatalf("stopped, E should let the player out (%q)", g.prompt.Noting())
	}
	g.tick(30)
	cc := get[physics.CharacterController](g, g.player)
	feet := get[transform.Transform](g, g.player).Translation
	car = get[transform.Transform](g, g.car).Translation
	if cc == nil || !cc.Grounded || rl.Vector3Distance(feet, car) < 1.2 {
		t.Fatalf("the player should stand on the ground beside it: at %v, buggy at %v", feet, car)
	}
	if g.driving.Active() {
		t.Fatal("on foot, the player isn't driving")
	}

	g.tick(150)
	if *get[physics.RigidBody](g, g.car) != physics.Static {
		t.Fatal("left at rest, it should be parked Static")
	}
	parked := get[transform.Transform](g, g.car).Translation
	g.tick(60)
	if get[transform.Transform](g, g.car).Translation != parked {
		t.Fatal("parked, it should stay put")
	}
}

func TestReverseFromAStop(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	g.tick(30)
	g.keys.Press(rl.KeyS)
	g.tick(60)
	if st := get[physics.VehicleState](g, g.car); st.Gear >= 0 {
		t.Fatalf("it should be in reverse, in gear %d", st.Gear)
	}
	g.tick(90)
	if z := get[transform.Transform](g, g.car).Translation.Z; z > -2 {
		t.Fatalf("S from a stop should reverse, it's at z=%v", z)
	}
}

func TestSteering(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	g.tick(30)
	g.keys.Press(rl.KeyW)
	g.keys.Press(rl.KeyD)
	g.tick(150)
	car := get[transform.Transform](g, g.car)
	// Facing +Z, its right is -X.
	if car.Translation.X > -2 {
		t.Fatalf("D should turn it right (toward -X), it's at %v", car.Translation)
	}
	st := get[physics.VehicleState](g, g.car)
	if st.Wheels[0].Steer >= 0 {
		t.Fatalf("its front wheels should be turned right (negative steer), got %v", st.Wheels[0].Steer)
	}
}

func TestExitBlocked(t *testing.T) {
	g := newRig(t)
	g.tap(rl.KeyE)
	g.tick(30)
	// Walls along both sides and behind: nowhere to stand.
	g.app.AddSystems(illusion.Update, illusion.Fn1(func(cmd *illusion.Commands) {
		for _, w := range []struct{ at, size rl.Vector3 }{
			{rl.Vector3{X: 1.8, Y: 1.5}, rl.Vector3{X: 1, Y: 3, Z: 8}},
			{rl.Vector3{X: -1.8, Y: 1.5}, rl.Vector3{X: 1, Y: 3, Z: 8}},
			{rl.Vector3{Y: 1.5, Z: -3}, rl.Vector3{X: 4, Y: 3, Z: 1}},
		} {
			cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(w.size.X, w.size.Y, w.size.Z)),
				illusion.C(transform.FromTranslation(w.at)))
		}
	}).RunIf(illusion.Once()))
	g.tick(10)
	g.tap(rl.KeyE)
	if !ecs.NewMap[character.Seated](g.app.World).Has(g.player) || g.prompt.Noting() != "No room to get out here" {
		t.Fatalf("hemmed in, the player should stay in, got note %q", g.prompt.Noting())
	}
}

// standIn is a rough stand-in for a kind of vehicle: wheels at (x, z) of
// radius r, all driven, the front ones steering, under a chassis box.
func standIn(handling string, r, width float32, wheels [][2]float32, chassis Box, seat [3]float32) *Spec {
	s := &Spec{Handling: handling, Chassis: []Box{chassis},
		Seats: []SeatSpec{{At: seat, Exits: [][3]float32{{seat[0] + 2.5, 0, seat[2]}}}}}
	for _, w := range wheels {
		s.Wheels = append(s.Wheels, WheelSpec{At: [3]float32{w[0], r, w[1]}, Radius: r, Width: width,
			Left: w[0] > 0, Steer: w[1] > 0, Drive: true, HandBrake: w[1] < 0})
	}
	return s
}

func TestEveryKindDrivesAndStaysUp(t *testing.T) {
	for name, spec := range map[string]*Spec{
		"trike": standIn("trike", 0.34, 0.22, [][2]float32{{0.44, 0.85}, {-0.44, 0.85}, {0, -0.95}},
			Box{Center: [3]float32{0, 0.75, -0.05}, Size: [3]float32{0.9, 0.6, 2.1}}, [3]float32{0, 0.62, -0.25}),
		"bike": standIn("bike", 0.33, 0.12, [][2]float32{{0, 0.74}, {0, -0.74}},
			Box{Center: [3]float32{0, 0.85, 0}, Size: [3]float32{0.35, 0.6, 1.6}}, [3]float32{0, 0.7, -0.15}),
		"rover": standIn("rover", 0.55, 0.4, [][2]float32{{1.4, 1.8}, {-1.4, 1.8}, {1.4, 0}, {-1.4, 0}, {1.4, -1.8}, {-1.4, -1.8}},
			Box{Center: [3]float32{0, 1.75, 0}, Size: [3]float32{2.4, 1.8, 5}}, [3]float32{0.5, 1.2, 1.4}),
		"truck": standIn("truck", 0.6, 0.45, [][2]float32{{1.1, 2.5}, {-1.1, 2.5}, {1.1, -1.3}, {-1.1, -1.3}, {1.1, -2.7}, {-1.1, -2.7}},
			Box{Center: [3]float32{0, 2.2, 0}, Size: [3]float32{2.4, 2.6, 7.4}}, [3]float32{0.6, 1.5, 2.6}),
	} {
		t.Run(name, func(t *testing.T) {
			g := newRigWith(t, name, spec)
			g.tap(rl.KeyE)
			if !g.driving.Active() {
				t.Fatalf("couldn't get in: prompt %+v", *g.prompt)
			}
			g.tick(60)
			start := get[transform.Transform](g, g.car).Translation
			if math.Abs(float64(start.Y)) > 0.03 {
				t.Errorf("woken, it should settle where it was parked, not at y=%v", start.Y)
			}
			g.keys.Press(rl.KeyW)
			g.tick(240)
			g.keys.Press(rl.KeyD)
			g.tick(180)
			tr := get[transform.Transform](g, g.car)
			up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, tr.Rotation)
			if up.Y < 0.7 {
				t.Fatalf("it went over: up is %v", up)
			}
			if d := rl.Vector3Distance(tr.Translation, start); d < 12 {
				t.Fatalf("it only went %v m", d)
			}
			if st := get[physics.VehicleState](g, g.car); st.Touching == 0 {
				t.Fatal("it's off the ground")
			}
		})
	}
}

// A parked vehicle's tyres are solid: a ray along its side, through the
// wheels, hits one. Driven, they're not in the way of its wheels.
func TestParkedTyresAreSolid(t *testing.T) {
	g := newRig(t)
	p := ecs.NewMap[Tyre](g.app.World)
	hit := func() (ecs.Entity, bool) {
		var found ecs.Entity
		var ok bool
		g.app.AddSystems(illusion.Update, illusion.Fn1(func(ph *physics.Physics) {
			// Along the buggy's left side at hub height, front to back.
			h, hit := ph.CastRay(rl.Vector3{X: 0.85, Y: 0.38, Z: 3}, rl.Vector3{Z: -1}, 6)
			if hit && p.Has(h.Entity) {
				found, ok = h.Entity, true
			}
		}).RunIf(illusion.Once()))
		g.tick(1)
		return found, ok
	}
	if _, ok := hit(); !ok {
		t.Fatal("parked, a ray along the wheels should hit a tyre")
	}
	g.tap(rl.KeyE)
	g.tick(10)
	if _, ok := hit(); ok {
		t.Fatal("driven, the tyres shouldn't be there")
	}
	if st := get[physics.VehicleState](g, g.car); st.Touching != 4 {
		t.Fatalf("its wheels should find the ground, not tyres: %d touch", st.Touching)
	}
}
