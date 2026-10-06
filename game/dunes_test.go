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

// dunes is a stretch of open dunes west of the Pads.
var dunes = rl.Vector3{X: 9600, Z: -1700}

// TestVehiclesKeepToTheDunes drives each vehicle over the dunes, straight
// and turning both ways, for half a minute: it should ride with the ground,
// not rock from side to side off it, nor roll over.
func TestVehiclesKeepToTheDunes(t *testing.T) {
	for _, piece := range []string{"buggy", "trike", "bike", "rover", "hauler", "hauler_tanker"} {
		t.Run(piece, func(t *testing.T) {
			worst, rocks := driveDunes(t, piece)
			t.Logf("%s: worst %.1f° off the ground, rocked side to side %d times", piece, worst, rocks)
			// A bike leans into its turns, as far as its lean goes, and a
			// dune's slope across a turn adds to that off the ground; the
			// rest keep to the ground but for their suspension.
			limit := 30.0
			if piece == "bike" {
				limit = float64(vehicle.Handlings["bike"].Lean.MaxAngle*rl.Rad2deg) + 15
			}
			// (Light vehicles hop dunes at speed, a side's wheels off the
			// ground for a moment; rocking that's a fault leans them far
			// off it, as anti-roll bars did.)
			if worst > limit {
				t.Errorf("it doesn't keep to the ground: %.1f° off it, rocking %d times", worst, rocks)
			}
		})
	}
}

// driveDunes gets the player into piece on the dunes and drives it, and
// returns how far it ever leaned off the ground under it, and how many
// times it rocked from standing on one side's wheels to the other's.
func driveDunes(t *testing.T, piece string) (float64, int) {
	t.Helper()
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	asset.RegisterLoader(app, stub[render.Model], nil)
	asset.RegisterLoader(app, stub[render.Animations], nil)
	asset.RegisterLoader(app, stub[render.Texture], nil)
	keys := input.NewButtonInput[input.Key]()
	app.InsertResource(illusion.R(&asset.Settings{Root: "../assets"}), illusion.R(keys),
		illusion.R(input.NewButtonInput[input.MouseButton]()), illusion.R(&input.Mouse{}))
	app.AddPlugins(character.Plugin{Models: []character.Model{{Path: "characters/man.glb", Scale: 1,
		Clips: map[character.Anim]character.Clip{character.Idle: {Name: "idle"}}}}},
		world.Plugin{Manifest: "world/world.json"}, vehicle.Plugin{})
	app.AddSystems(illusion.Startup, illusion.Fn3(func(cmd *illusion.Commands, kit *illusion.Res[world.Kit], r *illusion.Res[character.Roster]) {
		terrainAround(cmd, dunes.X, dunes.Z, 1)
		at := rl.Vector3{X: dunes.X, Y: walkHeight(dunes.X, dunes.Z), Z: dunes.Z}
		west := rl.QuaternionFromAxisAngle(transform.Up, -math.Pi/2)
		k := kit.Get()
		spec := k.Pieces[piece].Vehicle
		none := func(asset.Handle[render.Model]) []illusion.Component { return nil }
		if _, err := vehicle.Spawn(cmd, piece, spec, k.Model(piece), k.Model, none, at, west); err != nil {
			t.Fatal(err)
		}
		s := spec.Seats[0].At
		feet := rl.Vector3Add(at, rl.Vector3RotateByQuaternion(rl.Vector3{X: s[0], Z: s[2] - 1.6}, west))
		feet.Y = walkHeight(feet.X, feet.Z)
		r.Get().Spawn(cmd, 0, feet, illusion.C(character.Player{}))
	}))
	t.Cleanup(app.Cleanup)
	tick := func() { app.Tick(time.Second / 60) }
	for range 30 {
		tick()
	}
	var car ecs.Entity
	for q := ecs.NewFilter1[vehicle.Drivable](app.World).Query(); q.Next(); {
		car = q.Entity()
	}
	keys.Press(rl.KeyE)
	tick()
	keys.Release(rl.KeyE)
	keys.Clear()
	if !ecs.GetResource[vehicle.Driving](app.World).Active() {
		t.Fatalf("couldn't get into the %s", piece)
	}
	tr := ecs.NewMap[transform.Transform](app.World).Get(car)
	st := ecs.NewMap[physics.VehicleState](app.World).Get(car)
	keys.Press(rl.KeyW)
	worst, rocks, side := 0.0, 0, 0
	for i := range 1800 {
		switch i {
		case 300:
			keys.Press(rl.KeyA)
		case 500:
			keys.Release(rl.KeyA)
		case 800:
			keys.Press(rl.KeyD)
		case 1100:
			keys.Release(rl.KeyD)
			keys.Release(rl.KeyW)
		case 1300:
			keys.Press(rl.KeyW)
			keys.Press(rl.KeyA)
		}
		tick()
		up := rl.Vector3RotateByQuaternion(rl.Vector3{Y: 1}, tr.Rotation)
		x, z, e := tr.Translation.X, tr.Translation.Z, float32(0.5)
		ground := rl.Vector3Normalize(rl.Vector3{X: -(walkHeight(x+e, z) - walkHeight(x-e, z)) / (2 * e), Y: 1,
			Z: -(walkHeight(x, z+e) - walkHeight(x, z-e)) / (2 * e)})
		worst = math.Max(worst, math.Acos(math.Min(1, float64(rl.Vector3DotProduct(up, ground))))*180/math.Pi)
		left, right := 0, 0
		for j, w := range st.Wheels {
			if !w.Contact {
				continue
			}
			if spec := ecs.NewMap[vehicle.Drivable](app.World).Get(car).Spec; spec.Wheels[j].Left {
				left++
			} else if spec.Wheels[j].At[0] < 0 {
				right++
			}
		}
		s := 0
		if left > 0 && right == 0 {
			s = -1
		} else if right > 0 && left == 0 {
			s = 1
		}
		if s != 0 && s != side {
			if side != 0 {
				rocks++
			}
			side = s
		}
	}
	return worst, rocks
}
