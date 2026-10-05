// Shot draws some .glb files side by side as the game draws them (its toon
// shader and outlines, a warm light, the red ground) and saves a frame, for
// looking world pieces over in the game's own renderer:
//
//	go run ./tools/shot <out.png> <asset root> <eye x y z> <target x y z> <file.glb@x,z[,y[,turns]]> ...
//
// Paths are relative to the asset root; eye, target and positions are in
// metres, the game's frame (Y up); turns are quarter turns, as the layouts
// have them.
package main

import (
	"math"
	"os"
	"strconv"
	"strings"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/defaults"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

func f(s string) float32 { v, _ := strconv.ParseFloat(s, 32); return float32(v) }

func main() {
	a := os.Args
	out, root := a[1], a[2]
	eye := rl.Vector3{X: f(a[3]), Y: f(a[4]), Z: f(a[5])}
	target := rl.Vector3{X: f(a[6]), Y: f(a[7]), Z: f(a[8])}
	items := a[9:]
	frame := 0
	illusion.New().
		AddPlugins(
			defaults.Plugins(defaults.Config{Window: window.Config{Title: "shot", Width: 1280, Height: 720, MSAA: true}, AssetRoot: root}),
			shading.Plugin{
				ShadowColor: rl.NewColor(185, 165, 240, 255), Softness: 0.03, MidBand: 0.45,
				RimColor: rl.NewColor(120, 95, 60, 255), RimPower: 3, RimThreshold: 0.35,
				OutlineColor: rl.NewColor(60, 40, 55, 255), OutlineWidth: 1.5,
			},
		).
		InsertResource(
			illusion.R(&render.ClearColor{Color: rl.NewColor(250, 196, 120, 255)}),
			illusion.R(&render.AmbientLight{Color: rl.NewColor(225, 228, 245, 255), Brightness: 0.45}),
		).
		AddSystems(illusion.Startup, illusion.Fn4(func(cmd *illusion.Commands, meshes *illusion.Res[asset.Assets[render.Mesh]], mats *illusion.Res[asset.Assets[render.StandardMaterial]], models *asset.Loader[render.Model]) {
			cmd.Spawn(illusion.C(render.Camera3d{}), illusion.C(transform.FromTranslation(eye).LookingAt(target, transform.Up)))
			cmd.Spawn(illusion.C(render.DirectionalLight{Color: rl.NewColor(160, 142, 118, 255)}), illusion.C(transform.Identity().LookingAt(rl.Vector3{X: -2, Y: -2, Z: -1}, transform.Up)))
			cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Get().Add(render.Plane(80, 80))}), illusion.C(render.MeshMaterial3d{Material: mats.Get().Add(render.StandardMaterial{BaseColor: rl.NewColor(150, 82, 58, 255)})}), illusion.C(transform.FromXYZ(0, -.01, 0)))
			for _, it := range items {
				// file@x,z or file@x,z,y or file@x,z,y,turns (quarter turns, as
				// the layouts have them).
				path, at, _ := strings.Cut(it, "@")
				v := strings.Split(at, ",")
				y, turns := float32(0), float32(0)
				if len(v) > 2 {
					y = f(v[2])
				}
				if len(v) > 3 {
					turns = f(v[3])
				}
				tr := transform.FromXYZ(f(v[0]), y, f(v[1])).WithRotation(rl.QuaternionFromAxisAngle(transform.Up, turns*math.Pi/2))
				cmd.Spawn(illusion.C(render.Model3d{Model: models.MustLoad(path)}), illusion.C(tr), illusion.C(render.Passes{shading.OutlinePass(nil)}))
			}
		})).
		AddSystems(illusion.Render, illusion.Fn0(func() {
			frame++
			if frame == 30 {
				img := rl.LoadImageFromScreen()
				rl.ExportImage(*img, out)
				os.Exit(0)
			}
		}).InSet(render.Draw2D)).
		Run()
}
