package character

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func stub[T any](string) (T, error) { var v T; return v, nil }

// seatApp is a character standing on a floor, with no window: its model
// loads as nothing, but its animation player still takes clips.
func seatApp(t *testing.T) (*illusion.App, ecs.Entity, ecs.Entity) {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	asset.RegisterLoader(app, stub[render.Model], nil)
	asset.RegisterLoader(app, stub[render.Animations], nil)
	asset.RegisterLoader(app, stub[render.Texture], nil)
	app.InsertResource(
		illusion.R(&asset.Settings{Root: "../assets"}),
		illusion.R(input.NewButtonInput[input.Key]()),
		illusion.R(input.NewButtonInput[input.MouseButton]()),
		illusion.R(&input.Mouse{}),
	)
	app.AddPlugins(Plugin{Models: []Model{{Path: "characters/man.glb", Scale: 1, Clips: map[Anim]Clip{
		Idle: {Name: "idle"}, Drive: {Name: "drive"},
	}}}})
	app.AddSystems(illusion.Startup, illusion.Fn2(func(cmd *illusion.Commands, r *illusion.Res[Roster]) {
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(40, 1, 40)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		r.Get().Spawn(cmd, 0, rl.Vector3{}, illusion.C(Player{}))
	}))
	t.Cleanup(app.Cleanup)
	tick(app, 10)
	var root, body ecs.Entity
	for q := ecs.NewFilter1[Player](app.World).Query(); q.Next(); {
		root = q.Entity()
	}
	for q := ecs.NewFilter1[Body](app.World).Query(); q.Next(); {
		body = q.Entity()
	}
	return app, root, body
}

// do runs fn with commands once, on the next tick.
func do(app *illusion.App, fn func(cmd *illusion.Commands)) {
	app.AddSystems(illusion.Update, illusion.Fn1(fn).Before(Input).RunIf(illusion.Once()))
	tick(app, 1)
}

func tick(app *illusion.App, n int) {
	for range n {
		app.Tick(time.Second / 60)
	}
}

func TestSitAndStand(t *testing.T) {
	app, root, body := seatApp(t)
	seat := app.World.NewEntity()
	turned := rl.QuaternionFromAxisAngle(transform.Up, math.Pi/2)
	pose := transform.Transform{Translation: rl.Vector3{X: 5, Y: 0.6, Z: 2}, Rotation: turned, Scale: rl.Vector3One()}
	do(app, func(cmd *illusion.Commands) { Sit(cmd, root, Seated{Seat: seat, Pose: pose, Anim: Drive}) })
	tick(app, 2)

	if ecs.NewMap[physics.CharacterController](app.World).Has(root) {
		t.Fatal("a seated character shouldn't keep its capsule")
	}
	rootTr := ecs.NewMap[transform.Transform](app.World).Get(root)
	if d := rl.Vector3Distance(rootTr.Translation, rl.Vector3{X: 5, Y: 0.6 + capsuleHeight/2, Z: 2}); d > 1e-4 {
		t.Fatalf("seated root at %v, want the seat's pose raised by half a capsule", rootTr.Translation)
	}
	if rootTr.Rotation != turned {
		t.Fatal("a seated character should face the way its seat does")
	}
	st := ecs.NewMap[State](app.World).Get(body)
	player := ecs.NewMap[render.AnimationPlayer](app.World).Get(body)
	if st.Current != Drive || player.Clip() != "drive" {
		t.Fatalf("seated, it should play its drive clip, got %v playing %q", st.Current, player.Clip())
	}

	// The seat moves; the character goes with it.
	ecs.NewMap[Seated](app.World).Get(root).Pose.Translation.X = 9
	tick(app, 1)
	if x := ecs.NewMap[transform.Transform](app.World).Get(root).Translation.X; x != 9 {
		t.Fatalf("the character should ride with its seat, at x=%v", x)
	}

	do(app, func(cmd *illusion.Commands) { Stand(cmd, root, rl.Vector3{X: 11, Z: 2}, math.Pi/2) })
	tick(app, 30)
	cc := ecs.NewMap[physics.CharacterController](app.World).Get(root)
	if cc == nil || !cc.Grounded {
		t.Fatal("standing up, it should have its capsule back, on the floor")
	}
	if ecs.NewMap[Seated](app.World).Has(root) {
		t.Fatal("it's still seated")
	}
	rootTr = ecs.NewMap[transform.Transform](app.World).Get(root)
	if math.Abs(float64(rootTr.Translation.X-11)) > 0.05 || rootTr.Rotation != rl.QuaternionIdentity() {
		t.Fatalf("it should stand upright where it was put, at %v", rootTr.Translation)
	}
	if yaw := yawOf(ecs.NewMap[transform.Transform](app.World).Get(body).Rotation); math.Abs(float64(yaw-math.Pi/2)) > 1e-3 {
		t.Fatalf("it should face the way it was stood, yaw %v", yaw)
	}
	if st.Current == Drive {
		t.Fatal("standing, it should stop driving")
	}
}

func TestSittingOutOfSight(t *testing.T) {
	app, root, body := seatApp(t)
	seat := app.World.NewEntity()
	hidden := ecs.NewMap[render.Hidden](app.World)
	do(app, func(cmd *illusion.Commands) {
		Sit(cmd, root, Seated{Seat: seat, Pose: transform.Identity(), Anim: Drive, Hidden: true})
	})
	tick(app, 2)
	if !hidden.Has(body) {
		t.Fatal("seated inside, the body should be hidden")
	}
	do(app, func(cmd *illusion.Commands) { Stand(cmd, root, rl.Vector3{X: 3}, 0) })
	tick(app, 2)
	if hidden.Has(body) {
		t.Fatal("standing up, the body should show again")
	}
}
