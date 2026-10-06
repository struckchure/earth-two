package shading

import (
	"fmt"
	"image/color"
	"math"
	"sort"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// SpotLight shines along its transform's Forward direction. Angles are
// half angles in degrees: full brightness inside Inner, fading to Outer.
type SpotLight struct {
	Color                          color.RGBA
	Intensity, Range, Inner, Outer float32
	Enabled                        bool
	// Higher priority keeps the driver's beams available in a crowd.
	Priority int
}

const maxSpots = 8

type spot struct {
	light    SpotLight
	pose     transform.GlobalTransform
	distance float32
}

// gatherSpots runs after transforms, so beams follow the drawn vehicle
// between physics steps. It keeps the nearest lights in their own budget,
// leaving the world's point lights available to lamps and beacons.
func gatherSpots(
	lights *illusion.Query2[SpotLight, transform.GlobalTransform],
	cameras *illusion.Query2[render.Camera3d, transform.GlobalTransform],
	shader *illusion.Res[render.Shader],
) {
	u := shader.Get().Uniforms
	u["spotCount"] = []float32{0}
	var eye rl.Vector3
	var camera *render.Camera3d
	cameras.Each(func(_ ecs.Entity, c *render.Camera3d, tr *transform.GlobalTransform) {
		if camera == nil || c.Order > camera.Order {
			camera, eye = c, tr.Translation()
		}
	})
	if camera == nil {
		return
	}
	var nearby []spot
	lights.Each(func(_ ecs.Entity, l *SpotLight, tr *transform.GlobalTransform) {
		if !l.Enabled || l.Range <= 0 || l.Intensity <= 0 {
			return
		}
		d := rl.Vector3Distance(eye, tr.Translation())
		if d < l.Range+80 {
			nearby = append(nearby, spot{*l, *tr, d})
		}
	})
	sort.SliceStable(nearby, func(i, j int) bool {
		if nearby[i].light.Priority != nearby[j].light.Priority {
			return nearby[i].light.Priority > nearby[j].light.Priority
		}
		return nearby[i].distance < nearby[j].distance
	})
	nearby = nearby[:min(len(nearby), maxSpots)]
	u["spotCount"] = []float32{float32(len(nearby))}
	for i, s := range nearby {
		p, d, l := s.pose.Translation(), s.pose.Forward(), s.light
		u[fmt.Sprintf("spotPos[%d]", i)] = []float32{p.X, p.Y, p.Z, l.Range}
		u[fmt.Sprintf("spotDir[%d]", i)] = []float32{d.X, d.Y, d.Z}
		u[fmt.Sprintf("spotColor[%d]", i)] = scaled(l.Color, l.Intensity)
		u[fmt.Sprintf("spotCone[%d]", i)] = []float32{float32(math.Cos(float64(transform.Deg(l.Outer)))), float32(math.Cos(float64(transform.Deg(l.Inner))))}
	}
}
