// Package game is Earth Two: a free-roam role game in a science-fiction
// world, built with illusion. cmd/desktop runs it in a window, and cmd/web
// builds it for the browser.
//
// It opens on the title screen (see menu.go). In play, WASD or the arrow
// keys walk, Shift runs, Space jumps, E interacts, F punches, Q picks up, C
// opens the wardrobe (see wardrobe.go) and Esc pauses.
package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/shading"
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
	character.Idle:          {Name: "Breathing Idle"},
	character.Walk:          {Name: "Walking"},
	character.Run:           {Name: "Running"},
	character.Jump:          {Name: "Jumping", Start: 0.65, Land: 1.05},
	character.RunJump:       {Name: "Running Jump", Start: 0.13, Land: 0.62},
	character.Interact:      {Name: "Interact"},
	character.Punch:         {Name: "Punching"},
	character.PunchRight:    {Name: "Punching Mirrored"},
	character.PickUp:        {Name: "Picking Up"},
	character.Slide:         {Name: "Traversal_Slide"},
	character.Roll:          {Name: "Roll"},
	character.LadderClimb:   {Name: "Traversal_Ladder"},
	character.Vault:         {Name: "Traversal_Vault"},
	character.Mantle:        {Name: "Traversal_Mantle"},
	character.WallKick:      {Name: "Traversal_WallKick"},
	character.WallKickRight: {Name: "Traversal_WallKickRight"},
	character.WallFall:      {Name: "Traversal_WallKickFall"},
	character.WallFallRight: {Name: "Traversal_WallKickFallRight"},
	character.WallLand:      {Name: "Traversal_WallLand"},
	character.Crouch:        {Name: "Traversal_Crouch"},
	character.StandUp:       {Name: "Traversal_StandUp"},
	character.LadderExit:    {Name: "Traversal_LadderExit"},
	character.LadderEnter:   {Name: "Traversal_Ladder"},
	character.Fall:          {Name: "Traversal_Fall"},
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
				Window:    window.Config{Title: "Earth Two", Resizable: true, HighDPI: true, MSAA: true, VSync: true, KeepEscape: true},
				AssetRoot: assetRoot(),
			}),
			physics.Plugin{},
			character.Plugin{Models: people, Wardrobe: "characters/wardrobe.json", Outline: shading.OutlinePass},
			shading.Plugin{
				ShadowColor:  rl.NewColor(185, 165, 240, 255),
				Softness:     0.03,
				MidBand:      0.45,
				RimColor:     rl.NewColor(120, 95, 60, 255),
				RimPower:     3,
				RimThreshold: 0.35,
				OutlineColor: rl.NewColor(60, 40, 55, 255),
				OutlineWidth: 1.5,
			},
		).
		InsertResource(
			illusion.R(&render.ClearColor{Color: rl.NewColor(250, 196, 120, 255)}),
			illusion.R(&render.AmbientLight{Color: rl.NewColor(225, 228, 245, 255), Brightness: 0.45}),
			illusion.R(newMenu()),
			illusion.R(&uiFonts{}),
		).
		AddSystems(illusion.Startup, illusion.Fn6(setup)).
		AddSystems(illusion.Update,
			illusion.Chain(illusion.Fn8(menuInput), illusion.Fn4(lockControls), illusion.Fn5(faceCamera), illusion.Fn7(follow)),
			illusion.Fn1(respawn),
		).
		AddSystems(illusion.Render, illusion.Chain(illusion.Fn4(hud), illusion.Fn5(drawMenus)).InSet(render.Draw2D)).
		Run()
}

func setup(
	cmd *illusion.Commands,
	meshes *illusion.Res[asset.Assets[render.Mesh]],
	materials *illusion.Res[asset.Assets[render.StandardMaterial]],
	textures *asset.Loader[render.Texture],
	roster *illusion.Res[character.Roster],
	wardrobe *illusion.Res[character.Wardrobe],
) {
	m, mat := meshes.Get(), materials.Get()
	cmd.Spawn(
		illusion.C(render.Camera3d{}),
		illusion.C(transform.FromTranslation(cameraOffset).LookingAt(rl.Vector3{}, transform.Up)),
	)
	// A low evening sun.
	cmd.Spawn(
		illusion.C(render.DirectionalLight{Color: rl.NewColor(160, 142, 118, 255)}),
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

	traversalCourse(cmd, m, mat)
	roster.Get().Spawn(cmd, 0, rl.Vector3{}, illusion.C(character.Player{}), illusion.C(startingOutfit(wardrobe.Get())))
}

// startingOutfit is what the player first wears: whichever of these the
// first body's wardrobe has.
func startingOutfit(w *character.Wardrobe) character.Outfit {
	var o character.Outfit
	if len(w.Bodies) == 0 {
		return o
	}
	for slot, name := range map[character.Slot]string{
		character.Hair: "Short", character.Top: "T-shirt", character.Bottom: "Cargo pants", character.Shoes: "White trainers",
	} {
		if i, ok := w.Find(0, slot, name); ok {
			o.Put(slot, i)
		}
	}
	return o
}

// respawn puts characters that fell off the edge of the world back in the
// middle.
func respawn(q *illusion.Query3Where[transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Character]]) {
	q.Each(func(_ ecs.Entity, tr *transform.Transform, cc *physics.CharacterController, traversal *character.Traversal) {
		if tr.Translation.Y < -20 {
			tr.Translation = rl.Vector3{Y: 2}
			cc.Velocity = rl.Vector3{}
			cc.Walk = rl.Vector3{}
			cc.Controlled = false
			cc.Height = 1.8
			*traversal = character.Traversal{}
		}
	})
}
