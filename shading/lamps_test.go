package shading

import (
	"strings"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
)

func TestFixedLampsIncludeMoreThanCameraBudget(t *testing.T) {
	var lamps []Lamp
	for i := range 64 {
		lamps = append(lamps, Lamp{At: rl.Vector3{X: float32(i) * 300, Y: 4},
			Light: render.PointLight{Color: rl.White, Intensity: 2, Range: 12}})
	}
	app := illusion.New().AddPlugins(Plugin{Lamps: lamps})
	source := ecs.GetResource[render.Shader](app.World).Fragment
	if !strings.Contains(source, "const vec4 lampPos[64]") ||
		!strings.Contains(source, "vec4(18900.000000, 4.000000, 0.000000, 12.000000)") {
		t.Fatal("all fixed lamps, including distant ones, must reach the shader")
	}
	if !strings.Contains(source, "worldLamps(fragPosition, n)") {
		t.Fatal("surfaces must use their own positions to receive lamp light")
	}
	if strings.Contains(toonSurface, "pointCount") || strings.Contains(toonSurface, "pointLight(") {
		t.Fatal("toon surfaces still use the renderer's camera-selected lamps")
	}
}

func TestLampGroupsKeepEveryLightAndItsWholeReach(t *testing.T) {
	lamps := []Lamp{
		{At: rl.Vector3{X: 9800, Y: 6, Z: -1800}, Light: render.PointLight{Range: 18}},
		{At: rl.Vector3{X: -16, Y: 7, Z: 5}, Light: render.PointLight{Range: 12}},
		{At: rl.Vector3{X: 2, Y: 3, Z: 6}, Light: render.PointLight{Range: 8}},
		{At: rl.Vector3{X: -1500, Y: 9, Z: 12000}, Light: render.PointLight{Range: 14}},
	}
	ordered, groups := groupLamps(lamps)
	if len(ordered) != len(lamps) || len(groups) != 3 {
		t.Fatalf("lost lamps or mixed distant seats: %v, %v", ordered, groups)
	}
	seen := map[Lamp]int{}
	for _, g := range groups {
		for _, l := range ordered[g.start : g.start+g.count] {
			seen[l]++
			for _, axis := range []rl.Vector3{{X: 1}, {Y: 1}, {Z: 1}} {
				for _, sign := range []float32{-1, 1} {
					p := rl.Vector3Add(l.At, rl.Vector3Scale(axis, sign*l.Light.Range))
					if p.X < g.min.X || p.Y < g.min.Y || p.Z < g.min.Z || p.X > g.max.X || p.Y > g.max.Y || p.Z > g.max.Z {
						t.Fatalf("group would discard a surface lit by %v at %v", l, p)
					}
				}
			}
		}
	}
	for _, l := range lamps {
		if seen[l] != 1 {
			t.Fatalf("lamp occurs %d times: %v", seen[l], l)
		}
	}
	if source := lampGLSL(nil); strings.Contains(source, "[0]") {
		t.Fatal("empty worlds must not declare zero-sized GLSL arrays")
	}
}
