package game

import (
	"math"
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

// teleportRig is the player standing at from, beside the hire buggy, with
// a destination marked at to, and ground at both.
type teleportRig struct {
	app  *illusion.App
	keys *input.Keys
	wmap *worldMap
}

func newTeleportRig(t *testing.T, from, to rl.Vector2) *teleportRig {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	asset.RegisterLoader(app, stub[render.Model], nil)
	asset.RegisterLoader(app, stub[render.Animations], nil)
	asset.RegisterLoader(app, stub[render.Texture], nil)
	r := &teleportRig{app: app, keys: input.NewButtonInput[input.Key](),
		wmap: &worldMap{dest: to, marked: true, bounds: rl.Rectangle{X: -worldSize / 2, Y: -worldSize / 2, Width: worldSize, Height: worldSize}}}
	app.InsertResource(illusion.R(&asset.Settings{Root: "../assets"}), illusion.R(r.keys),
		illusion.R(input.NewButtonInput[input.MouseButton]()), illusion.R(&input.Mouse{}),
		illusion.R(&menu{}), illusion.R(r.wmap), illusion.R(newOrbit()))
	app.AddPlugins(character.Plugin{Models: []character.Model{{Path: "characters/man.glb", Scale: 1,
		Clips: map[character.Anim]character.Clip{character.Idle: {Name: "idle"}}}}},
		world.Plugin{Manifest: "world/world.json"}, vehicle.Plugin{}, residentsPlugin{}, testPlugin{})
	app.AddSystems(illusion.Startup, illusion.Fn3(func(cmd *illusion.Commands, kit *illusion.Res[world.Kit], roster *illusion.Res[character.Roster]) {
		terrainAround(cmd, from.X, from.Y, 1)
		terrainAround(cmd, to.X, to.Y, 1)
		placed, err := world.Layout("../assets", "world/landfall.json")
		if err != nil {
			t.Fatal(err)
		}
		if err := spawnOnTerrain(cmd, kit.Get(), placed); err != nil {
			t.Error(err)
		}
		roster.Get().Spawn(cmd, 0, rl.Vector3{X: from.X, Y: walkHeight(from.X, from.Y), Z: from.Y}, illusion.C(character.Player{}))
	}))
	t.Cleanup(app.Cleanup)
	r.tick(30)
	return r
}

func (r *teleportRig) tick(n int) {
	for range n {
		r.app.Tick(time.Second / 60)
	}
}

func (r *teleportRig) tap(k input.Key) {
	r.keys.Press(k)
	r.tick(1)
	r.keys.Release(k)
	r.keys.Clear()
	r.tick(1)
}

func (r *teleportRig) player() (*transform.Transform, *physics.CharacterController) {
	q := ecs.NewFilter1[character.Player](r.app.World).Query()
	var e ecs.Entity
	for q.Next() {
		e = q.Entity()
	}
	return ecs.NewMap[transform.Transform](r.app.World).Get(e), ecs.NewMap[physics.CharacterController](r.app.World).Get(e)
}

// P takes the player on foot to the destination marked on the map, a long
// way off, and they stand on the ground there.
func TestTeleportToTheDestination(t *testing.T) {
	hold := rl.Vector2{X: -1500, Y: 11900}
	r := newTeleportRig(t, rl.Vector2{X: hireBuggy.X + 6, Y: hireBuggy.Z + 6}, hold)
	r.tap(teleportKey)
	r.tick(60)
	tr, cc := r.player()
	feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
	if d := math.Hypot(float64(feet.X-hold.X), float64(feet.Z-hold.Y)); d > 1 {
		t.Fatalf("P should take the player to the destination, they're %.0f m off it", d)
	}
	if !cc.Grounded || math.Abs(float64(feet.Y-walkHeight(hold.X, hold.Y))) > .3 {
		t.Fatalf("they should stand on the ground there: feet at %.2f, ground at %.2f, grounded %v", feet.Y, walkHeight(hold.X, hold.Y), cc.Grounded)
	}
}

// Driving, P takes the vehicle with the player in it.
func TestTeleportWhileDriving(t *testing.T) {
	dunes := rl.Vector2{X: 9600, Y: -1700}
	r := newTeleportRig(t, rl.Vector2{X: hireBuggy.X + 0.4, Y: hireBuggy.Z + 2.9}, dunes)
	r.tap(rl.KeyE)
	d := ecs.GetResource[vehicle.Driving](r.app.World)
	if !d.Active() {
		t.Fatal("couldn't get into the buggy")
	}
	r.tap(teleportKey)
	r.tick(90)
	car := ecs.NewMap[transform.Transform](r.app.World).Get(d.Vehicle)
	if dist := math.Hypot(float64(car.Translation.X-dunes.X), float64(car.Translation.Z-dunes.Y)); dist > 1 {
		t.Fatalf("the buggy should have come too, it's %.0f m off", dist)
	}
	if st := ecs.NewMap[physics.VehicleState](r.app.World).Get(d.Vehicle); st.Touching != 4 {
		t.Fatalf("it should land on its wheels, %d touch", st.Touching)
	}
	if !d.Active() {
		t.Fatal("the player should still be driving it")
	}
}
