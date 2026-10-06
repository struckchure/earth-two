// Package game is Earth Two: a free-roam role game in a science-fiction
// world, built with illusion. cmd/desktop runs it in a window, and cmd/web
// builds it for the browser.
//
// It opens on the title screen (see menu.go). In play, WASD or the arrow
// keys walk, Shift runs, Space jumps, E interacts, F punches, Q picks up and
// Esc pauses; the wardrobe (see wardrobe.go) is in the menus.
package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/defaults"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

const (
	// cameraLag is how quickly the camera catches up with the player; higher
	// is tighter.
	cameraLag = 5
	// shadowRange is half the width of the square round the view where
	// things cast shadows, in metres.
	shadowRange = 45
)

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
	// Mixamo's Ascending and Descending Stairs, their feet put on the kit's
	// steps.
	character.StairsUp:   {Name: "Traversal_StairsUp", Step: stairStep},
	character.StairsDown: {Name: "Traversal_StairsDown", Step: stairDownStep},
	// In a vehicle's seat (Ride, astride a bike or a trike, falls back to
	// it): the library's driving loop.
	character.Drive: {Name: "Driving_Loop"},
}

// stairStep and stairDownStep are where in the stair clips' cycles a foot
// lands on a step's edge (tools/makehuman/traversal.py): going up, the left
// foot lands 0.175 of the way in, 0.22 m above the ground under the body;
// coming down, 5/28 of the way in, 0.209 m below it (the stairs' 0.3 m rise
// over its 0.696 of the cycle planted, from ahead to behind the body).
const (
	stairStep     = .54
	stairDownStep = .527
)

// people are the character models in assets/characters (see CREDITS.txt).
// They're made in metres, so they keep their own heights.
var people = []character.Model{
	{Path: "characters/man.glb", Clips: makehuman, Scale: 1},
	{Path: "characters/woman.glb", Clips: makehuman, Scale: 1},
}

// Run starts the game and blocks until its window closes.
func Run() { build(newMenu()).Run() }

// build is the game, opening on m.
func build(m *menu) *illusion.App {
	return illusion.New().
		AddPlugins(
			defaults.Plugins(defaults.Config{
				Window:    window.Config{Title: "Earth Two", Resizable: true, HighDPI: true, MSAA: true, VSync: true, KeepEscape: true},
				AssetRoot: assetRoot(),
			}),
			physics.Plugin{},
			character.Plugin{Models: people, Wardrobe: "characters/wardrobe.json", Outline: shading.OutlinePass},
			world.Plugin{Manifest: "world/world.json", Outline: shading.OutlinePass},
			vehicle.Plugin{},
			shading.Plugin{
				ShadowColor: rl.NewColor(185, 165, 240, 255),
				Softness:    0.03,
				// Low, so the ground under the low sun is in its full light.
				MidBand: 0.2,
				// No rim light: it follows the camera, so it reads as a
				// light carried round with the player.
				RimColor:     rl.NewColor(0, 0, 0, 255),
				RimPower:     3,
				RimThreshold: 0.35,
				OutlineColor: rl.NewColor(60, 40, 55, 255),
				OutlineWidth: 1.5,
				// Dust in the air: the Fringe hazes into the sky's horizon
				// with distance, so the seats loom out of it.
				FogColor:    skyHorizon,
				FogDistance: 3500,
				FogEnd:      bodyDistance - 200,
			},
		).
		InsertResource(
			illusion.R(&render.ClearColor{Color: rl.NewColor(250, 196, 120, 255)}),
			// The fill from the dusty sky: dimmer than the sun, and violet.
			illusion.R(&render.AmbientLight{Color: rl.NewColor(206, 186, 222, 255), Brightness: 0.26}),
			// The sun's shadows, round what the camera looks at.
			illusion.R(&render.Shadows{Size: 4096, Range: shadowRange}),
			illusion.R(m),
			illusion.R(newOrbit()),
			illusion.R(&vehicle.Ground{}),
			illusion.R(&uiFonts{}),
		).
		AddSystems(illusion.Startup, illusion.Fn7(setup)).
		AddSystems(illusion.Update,
			// After the characters act, so the camera follows a vehicle where
			// it's drawn this frame.
			illusion.Chain(illusion.Fn8(menuInput), illusion.Fn8(mapInput), illusion.Fn4(lockControls), illusion.Fn8(steerCamera), illusion.Fn4(steerDriving), illusion.Fn5(faceCamera), illusion.Fn8(follow), illusion.Fn5(streamTerrain), illusion.Fn2(coverGround), illusion.Fn8(cull), illusion.Fn7(cullVehicles), illusion.Fn3(moveSky)).After(character.Act),
			illusion.Fn1(respawn),
		).
		AddSystems(illusion.Render, illusion.Chain(illusion.Fn6(hud), illusion.Fn6(drawMaps), illusion.Fn5(drawMenus)).InSet(render.Draw2D))
}

func setup(
	cmd *illusion.Commands,
	meshes *illusion.Res[asset.Assets[render.Mesh]],
	materials *illusion.Res[asset.Assets[render.StandardMaterial]],
	roster *illusion.Res[character.Roster],
	wardrobe *illusion.Res[character.Wardrobe],
	kit *illusion.Res[world.Kit],
	textures *illusion.Res[asset.Assets[render.Texture]],
) {
	m, mat := meshes.Get(), materials.Get()
	setClipPlanes()
	cmd.Spawn(
		illusion.C(render.Camera3d{}),
		illusion.C(transform.FromTranslation(newOrbit().shot().offset).LookingAt(rl.Vector3{}, transform.Up)),
	)
	// The sun, TRAPPIST-1, low over Landfall, and the sky round it (see
	// sky.go).
	cmd.Spawn(
		illusion.C(render.DirectionalLight{Color: sunlight, Brightness: sunBrightness}),
		illusion.C(transform.Identity().LookingAt(rl.Vector3Negate(sunFrom), transform.Up)),
	)
	spawnSky(cmd, m, mat, textures.Get())
	// The Red under the seats and out across the Fringe, round where the
	// player arrives (see terrain.go). Floors have no colliders of their
	// own: this is what everyone walks on.
	cmd.InsertResource(illusion.R(spawnTerrain(cmd, m, mat, textures.Get(), arrival)))
	placed, err := world.Layout(assetRoot(), "world/landfall.json")
	if err != nil {
		panic(err)
	}
	if err := spawnOnTerrain(cmd, kit.Get(), placed); err != nil {
		panic(err)
	}
	cmd.InsertResource(illusion.R(newWorldMap(kit.Get(), placed)))
	roster.Get().Spawn(cmd, 0, arrival, illusion.C(character.Player{}), illusion.C(startingOutfit(wardrobe.Get())))
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

// floored is where floors cover the ground, on the XZ plane (X across, Y
// for the game's Z): the Hull's deck at Landfall, and the Pads' tiles out
// at the spaceport. They're HULL_* and PADS_* (moved to PADS_AT) in
// tools/world/landfall.py, in the game's frame.
var floored = []rl.Rectangle{
	{X: -40, Y: -12, Width: 48, Height: 32},
	{X: 9776, Y: -1830, Width: 44, Height: 64},
}

// arrival is where the player starts, and comes back to: on the Pads at
// the foot of the drifter's ramp, where new players arrive. It's SPAWN in
// tools/world/landfall.py, in the game's frame.
var arrival = rl.Vector3{X: 9807, Z: -1770.5}

// respawn puts characters that fell off the edge of the world back where
// the player arrives.
func respawn(q *illusion.Query3Where[transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Character]]) {
	q.Each(func(_ ecs.Entity, tr *transform.Transform, cc *physics.CharacterController, traversal *character.Traversal) {
		if tr.Translation.Y < -20 {
			tr.Translation = rl.Vector3Add(arrival, rl.Vector3{Y: 2})
			cc.Velocity = rl.Vector3{}
			cc.Walk = rl.Vector3{}
			cc.Controlled = false
			cc.Height = 1.8
			*traversal = character.Traversal{}
		}
	})
}
