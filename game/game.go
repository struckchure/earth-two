// Package game is Earth Two: a free-roam role game in low-poly Lagos, built
// with illusion. cmd/desktop runs it in a window, and cmd/web builds it for
// the browser.
//
// WASD or the arrow keys walk, Shift runs, Space jumps, E interacts, F
// punches, Q picks up and Tab switches between the man and the woman.
package game

import (
	"fmt"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/defaults"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

const (
	groundSize = 40
	// cameraLag is how quickly the camera catches up with the player; higher
	// is tighter.
	cameraLag = 5
)

// cameraOffset is where the camera sits relative to the player.
var cameraOffset = rl.Vector3{Y: 3.5, Z: 6}

// makehuman is the clip table for the people tools/makehuman builds, all
// retargeted onto MakeHuman's game engine rig: Mixamo's motion capture for
// the everyday moves, and Quaternius's Universal Animation Library for the
// rest. Jumps run from take-off to touchdown, timed from frame strips.
var makehuman = map[character.Anim]character.Clip{
	character.Idle:     {Name: "Breathing Idle"},
	character.Walk:     {Name: "Walking"},
	character.Run:      {Name: "Running"},
	character.Jump:     {Name: "Jumping", Start: 0.65, Land: 1.05},
	character.RunJump:  {Name: "Running Jump", Start: 0.13, Land: 0.62},
	character.Interact: {Name: "Interact"},
	character.Punch:    {Name: "Punching"},
	character.PickUp:   {Name: "Picking Up"},
}

// people are the character models in assets/characters (see CREDITS.txt).
// They're made in metres, so they keep their own heights.
var people = []character.Model{
	{Path: "characters/man.glb", Clips: makehuman, Scale: 1},
	{Path: "characters/woman.glb", Clips: makehuman, Scale: 1},
}

// Run starts the game and blocks until its window closes.
func Run() {
	illusion.New().
		AddPlugins(
			defaults.Plugins(defaults.Config{
				Window:    window.Config{Title: "Earth Two", Resizable: true, HighDPI: true, MSAA: true},
				AssetRoot: "assets",
			}),
			physics.Plugin{},
			character.Plugin{Models: people},
		).
		InsertResource(
			illusion.R(&render.ClearColor{Color: rl.NewColor(250, 196, 120, 255)}),
			illusion.R(&render.AmbientLight{Color: rl.NewColor(255, 214, 170, 255), Brightness: 0.45}),
		).
		AddSystems(illusion.Startup, illusion.Fn5(setup)).
		AddSystems(illusion.Update, illusion.Fn3(follow), illusion.Fn1(respawn)).
		AddSystems(illusion.Render, illusion.Fn1(hud).InSet(render.Draw2D)).
		Run()
}

func setup(
	cmd *illusion.Commands,
	meshes *illusion.Res[asset.Assets[render.Mesh]],
	materials *illusion.Res[asset.Assets[render.StandardMaterial]],
	textures *asset.Loader[render.Texture],
	roster *illusion.Res[character.Roster],
) {
	m, mat := meshes.Get(), materials.Get()
	cmd.Spawn(
		illusion.C(render.Camera3d{}),
		illusion.C(transform.FromTranslation(cameraOffset).LookingAt(rl.Vector3{}, transform.Up)),
	)
	// A low evening sun.
	cmd.Spawn(
		illusion.C(render.DirectionalLight{Color: rl.NewColor(255, 222, 180, 255)}),
		illusion.C(transform.Identity().LookingAt(rl.Vector3{X: -2, Y: -2, Z: -1}, transform.Up)),
	)
	cmd.Spawn(
		illusion.C(render.Mesh3d{Mesh: m.Add(render.Plane(groundSize, groundSize))}),
		// Loaded from assets/checker.png, which web builds bundle into the page.
		illusion.C(render.MeshMaterial3d{Material: mat.Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.MustLoad("checker.png")})}),
		illusion.C(transform.Identity()),
		illusion.C(physics.Static),
		illusion.C(physics.Cuboid(groundSize, 1, groundSize).WithOffset(rl.Vector3{Y: -0.5})),
	)

	roster.Get().Spawn(cmd, 0, rl.Vector3{}, illusion.C(character.Player{}))
}

// follow eases the camera along behind the player.
func follow(
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	cameras *illusion.Query1Where[transform.Transform, illusion.With[render.Camera3d]],
	t *illusion.Res[illusion.Time],
) {
	_, player, ok := players.Single()
	if !ok {
		return
	}
	target := player.Translation
	k := min(1, cameraLag*t.Get().DeltaSecs())
	cameras.Each(func(_ ecs.Entity, tr *transform.Transform) {
		tr.Translation = rl.Vector3Lerp(tr.Translation, rl.Vector3Add(target, cameraOffset), k)
		tr.LookAt(rl.Vector3Add(target, rl.Vector3{Y: 1}), transform.Up)
	})
}

// respawn puts characters that fell off the edge of the world back in the
// middle.
func respawn(q *illusion.Query2Where[transform.Transform, physics.CharacterController, illusion.With[character.Character]]) {
	q.Each(func(_ ecs.Entity, tr *transform.Transform, cc *physics.CharacterController) {
		if tr.Translation.Y < -20 {
			tr.Translation = rl.Vector3{Y: 2}
			cc.Velocity = rl.Vector3{}
		}
	})
}

// hud draws the frame rate and the controls, scaled for the screen.
func hud(win *illusion.Res[window.Window]) {
	s := win.Get().Scale
	px := func(v float32) int32 { return int32(v * s) }
	rl.DrawText(fmt.Sprintf("%d FPS", rl.GetFPS()), px(10), px(10), px(20), rl.Lime)
	rl.DrawText("WASD / arrows: walk   shift: run   space: jump   E: interact   F: punch   Q: pick up   tab: switch character", px(10), px(36), px(20), rl.RayWhite)
}
