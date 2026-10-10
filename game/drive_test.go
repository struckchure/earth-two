package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// hireBuggy is where Landfall parks the buggy for hire at the Pads.
var hireBuggy = rl.Vector3{X: 9761, Z: -1788}

// TestDriveTheHireBuggy walks up to the buggy at the Pads, gets in, drives
// it out across the ground and back to a stop, and gets out: Landfall's own
// buggy, on its terrain, among its pieces.
func TestDriveTheHireBuggy(t *testing.T) {
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
		world.Plugin{Manifest: "world/world.json"}, vehicle.Plugin{})
	feet := rl.Vector3Add(hireBuggy, rl.Vector3{X: 0.4, Z: 2.9})
	feet.Y = walkHeight(feet.X, feet.Z)
	app.AddSystems(illusion.Startup, illusion.Fn3(func(cmd *illusion.Commands, kit *illusion.Res[world.Kit], r *illusion.Res[character.Roster]) {
		terrainAround(cmd, feet.X, feet.Z, 1)
		placed, err := world.Layout("../assets", "world/landfall.json")
		if err != nil {
			t.Fatal(err)
		}
		if err := spawnOnTerrain(cmd, kit.Get(), placed, world.Stores{}); err != nil {
			t.Error(err)
		}
		r.Get().Spawn(cmd, 0, feet, illusion.C(character.Player{}))
	}))
	t.Cleanup(app.Cleanup)
	tick := func(n int) {
		for range n {
			app.Tick(time.Second / 60)
		}
	}
	tap := func(k input.Key) {
		keys.Press(k)
		tick(1)
		keys.Release(k)
		keys.Clear()
		tick(1)
	}
	tick(30)

	var buggy, player ecs.Entity
	for q := ecs.NewFilter2[vehicle.Drivable, transform.Transform](app.World).Query(); q.Next(); {
		d, tr := q.Get()
		if d.Name == "buggy" && rl.Vector3Distance(tr.Translation, hireBuggy) < 5 {
			buggy = q.Entity()
		}
	}
	for q := ecs.NewFilter1[character.Player](app.World).Query(); q.Next(); {
		player = q.Entity()
	}
	if buggy.IsZero() {
		t.Fatal("no buggy for hire at the Pads")
	}
	prompt := ecs.GetResource[vehicle.Prompt](app.World)
	if prompt.Text != "Drive the buggy" {
		t.Fatalf("beside it, the player should be offered the buggy, got %+v", *prompt)
	}
	parked := *ecs.NewMap[transform.Transform](app.World).Get(buggy)

	tap(rl.KeyE)
	if !ecs.NewMap[character.Seated](app.World).Has(player) {
		t.Fatal("E should seat the player in the buggy")
	}
	tick(90)
	tr := ecs.NewMap[transform.Transform](app.World).Get(buggy)
	if d := rl.Vector3Distance(tr.Translation, parked.Translation); d > 0.05 {
		t.Errorf("woken, it should settle where it was parked, but moved %.3f m", d)
	}
	st := ecs.NewMap[physics.VehicleState](app.World).Get(buggy)
	if st.Touching != 4 {
		t.Fatalf("it should stand on its 4 wheels on the Pads, %d touch", st.Touching)
	}

	keys.Press(rl.KeyW)
	tick(150)
	keys.Release(rl.KeyW)
	if d := rl.Vector3Distance(tr.Translation, parked.Translation); d < 6 {
		t.Fatalf("W should drive it off, it went %.1f m", d)
	}
	keys.Press(rl.KeyS)
	for range 400 {
		tick(1)
		if st.Speed < 0.5 {
			break
		}
	}
	keys.Release(rl.KeyS)
	if up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, tr.Rotation); up.Y < 0.9 {
		t.Fatalf("it should be on its wheels, up is %v", up)
	}
	tap(rl.KeyE)
	tick(30)
	if ecs.NewMap[character.Seated](app.World).Has(player) {
		t.Fatalf("stopped, E should let the player out (%q)", prompt.Noting())
	}
	tick(150)
	if *ecs.NewMap[physics.RigidBody](app.World).Get(buggy) != physics.Static {
		t.Fatal("left at rest, the buggy should park")
	}
}
