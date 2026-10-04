package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// traversalCourse is small enough to explore from the original spawn.
func traversalCourse(cmd *illusion.Commands, meshes *asset.Assets[render.Mesh], materials *asset.Assets[render.StandardMaterial]) {
	box := func(x, y, z, w, h, d float32, color rl.Color) {
		cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Add(render.Cuboid(w, h, d))}), illusion.C(render.MeshMaterial3d{Material: materials.Add(render.StandardMaterial{BaseColor: color})}), illusion.C(render.Passes{shading.BoxOutlinePass()}), illusion.C(transform.FromXYZ(x, y, z)), illusion.C(physics.Static), illusion.C(physics.Cuboid(w, h, d)))
	}
	amber := rl.NewColor(211, 133, 65, 255)
	blue := rl.NewColor(76, 111, 146, 255)
	gray := rl.NewColor(90, 95, 101, 255)
	// Vault barriers and a higher mantle ledge.
	box(0, .375, -4, 3, .75, .5, amber)
	box(4, .8, -5, 3, 1.6, 3, blue)
	// Ladder platform, approached from the front (+Z).
	// Leave room behind the rungs for shoes and bent knees. A ladder flush
	// against the platform wall cannot accommodate the climb pose.
	box(-5, 1.4, -6.2, 3, 2.8, 2, blue)
	bottom := rl.Vector3{X: -5, Y: 0, Z: -4.55}
	top := rl.Vector3{X: -5, Y: 2.84, Z: -4.55}
	cmd.Spawn(illusion.C(character.Ladder{Bottom: bottom, Top: top, Facing: rl.Vector3{Z: -1}, BottomExit: rl.Vector3{X: -5, Z: -3.9}, TopExit: rl.Vector3{X: -5, Y: 2.84, Z: -5.55}, Width: .64, RungSpacing: .3}))
	for _, x := range []float32{-5.24, -4.76} {
		box(x, 1.45, -4.92, .07, 2.9, .07, amber)
	}
	for y := float32(.3); y < 2.9; y += .3 {
		box(-5, y, -4.92, .55, .045, .06, amber)
	}
	// Parallel walls permit kicks between distinct surfaces: a long corridor
	// 4 m high and 2 m across, with room for a run-up along it and a
	// string of kicks up it. The far side is a block 3 m deep, so its top is
	// somewhere to climb out onto.
	box(8, 2, 4, .4, 4, 16, gray)
	box(11.7, 2, 4, 3, 4, 16, gray)
	// A slide passage with open standing room at both ends.
	box(-5, 1.2, 3, 3, .25, 3, amber)
	box(-6.45, .55, 3, .1, 1.1, 3, gray)
	box(-3.55, .55, 3, .1, 1.1, 3, gray)
}
