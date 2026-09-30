// Package game is Earth Two: a 3D game built with illusion. cmd/desktop runs
// it in a window, and cmd/web builds it for the browser.
//
// WASD or the arrow keys move the cube; Space drops a crate to push around.
package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/defaults"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

const speed = 5 // units per second

// cameraOffset is where the camera sits relative to the player.
var cameraOffset = rl.Vector3{Y: 6, Z: 9}

// Player is the cube you move.
type Player struct{}

// Crates is a resource with what dropped crates are made of.
type Crates struct {
	Mesh     asset.Handle[render.Mesh]
	Material asset.Handle[render.StandardMaterial]
}

// Run starts the game and blocks until its window closes.
func Run() {
	illusion.New().
		AddPlugins(
			defaults.Plugins(defaults.Config{
				Window:    window.Config{Title: "Earth Two", MSAA: true},
				AssetRoot: "assets",
			}),
			physics.Plugin{},
		).
		AddSystems(illusion.Startup, illusion.Fn4(setup)).
		AddSystems(illusion.Update, illusion.Chain(illusion.Fn3(move), illusion.Fn2(follow)), illusion.Fn4(dropCrate)).
		AddSystems(illusion.Render, illusion.Fn0(hud).InSet(render.Draw2D)).
		Run()
}

func setup(
	cmd *illusion.Commands,
	meshes *illusion.Res[asset.Assets[render.Mesh]],
	materials *illusion.Res[asset.Assets[render.StandardMaterial]],
	textures *asset.Loader[render.Texture],
) {
	m, mat := meshes.Get(), materials.Get()
	cmd.Spawn(
		illusion.C(render.Camera3d{}),
		illusion.C(transform.FromTranslation(cameraOffset).LookingAt(rl.Vector3{}, transform.Up)),
	)
	cmd.Spawn(
		illusion.C(render.DirectionalLight{Color: rl.White}),
		illusion.C(transform.Identity().LookingAt(rl.Vector3{X: -1, Y: -3, Z: -2}, transform.Up)),
	)
	cmd.Spawn(
		illusion.C(render.Mesh3d{Mesh: m.Add(render.Plane(20, 20))}),
		// Loaded from assets/checker.png, which web builds bundle into the page.
		illusion.C(render.MeshMaterial3d{Material: mat.Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.MustLoad("checker.png")})}),
		illusion.C(transform.Identity()),
		illusion.C(physics.Static),
		illusion.C(physics.Cuboid(20, 1, 20).WithOffset(rl.Vector3{Y: -0.5})),
	)
	cmd.Spawn(
		illusion.C(Player{}),
		illusion.C(render.Mesh3d{Mesh: m.Add(render.Cuboid(1, 1, 1))}),
		illusion.C(render.MeshMaterial3d{Material: mat.Add(render.StandardMaterial{BaseColor: rl.Orange})}),
		illusion.C(transform.FromXYZ(0, 0.5, 0)),
		// Kinematic: it follows its Transform and pushes crates out of the way.
		illusion.C(physics.Kinematic),
		illusion.C(physics.Cuboid(1, 1, 1)),
	)
	cmd.InsertResource(illusion.R(&Crates{
		Mesh:     m.Add(render.Cuboid(0.8, 0.8, 0.8)),
		Material: mat.Add(render.StandardMaterial{BaseColor: rl.Brown}),
	}))
}

func move(
	q *illusion.Query1Where[transform.Transform, illusion.With[Player]],
	keys *illusion.Res[input.Keys],
	t *illusion.Res[illusion.Time],
) {
	k := keys.Get()
	var dir rl.Vector3
	if k.AnyPressed(rl.KeyA, rl.KeyLeft) {
		dir.X--
	}
	if k.AnyPressed(rl.KeyD, rl.KeyRight) {
		dir.X++
	}
	if k.AnyPressed(rl.KeyW, rl.KeyUp) {
		dir.Z--
	}
	if k.AnyPressed(rl.KeyS, rl.KeyDown) {
		dir.Z++
	}
	if dir == (rl.Vector3{}) {
		return
	}
	step := rl.Vector3Scale(rl.Vector3Normalize(dir), speed*t.Get().DeltaSecs())
	q.Each(func(_ ecs.Entity, tr *transform.Transform) {
		tr.Translation.X = rl.Clamp(tr.Translation.X+step.X, -9.5, 9.5)
		tr.Translation.Z = rl.Clamp(tr.Translation.Z+step.Z, -9.5, 9.5)
	})
}

// follow keeps the camera behind the player.
func follow(
	players *illusion.Query1Where[transform.Transform, illusion.With[Player]],
	cameras *illusion.Query1Where[transform.Transform, illusion.With[render.Camera3d]],
) {
	_, player, ok := players.Single()
	if !ok {
		return
	}
	target := player.Translation
	cameras.Each(func(_ ecs.Entity, tr *transform.Transform) {
		tr.Translation = rl.Vector3Add(target, cameraOffset)
		tr.LookAt(target, transform.Up)
	})
}

// dropCrate drops a crate next to the player when Space is pressed.
func dropCrate(
	cmd *illusion.Commands,
	keys *illusion.Res[input.Keys],
	crates *illusion.Res[Crates],
	players *illusion.Query1Where[transform.Transform, illusion.With[Player]],
) {
	if !keys.Get().JustPressed(rl.KeySpace) {
		return
	}
	_, player, ok := players.Single()
	if !ok {
		return
	}
	c := crates.Get()
	cmd.Spawn(
		illusion.C(render.Mesh3d{Mesh: c.Mesh}),
		illusion.C(render.MeshMaterial3d{Material: c.Material}),
		illusion.C(transform.FromXYZ(player.Translation.X, 5, player.Translation.Z-2)),
		illusion.C(physics.Dynamic),
		illusion.C(physics.Cuboid(0.8, 0.8, 0.8)),
	)
}

func hud() {
	rl.DrawFPS(10, 10)
	rl.DrawText("WASD / arrows: move   space: drop a crate", 10, 36, 20, rl.RayWhite)
}
