package shading

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func TestSpotlightsFollowTransformsAndClearWhenOff(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{})
	shader := &render.Shader{Uniforms: map[string][]float32{}}
	app.InsertResource(illusion.R(shader))
	app.AddSystems(illusion.PostUpdate, illusion.Fn3(gatherSpots).After(transform.Propagate))
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(render.Camera3d{}), illusion.C(transform.FromXYZ(1000, 0, 0)))
		cmd.Spawn(illusion.C(render.Camera3d{Order: 1}), illusion.C(transform.Identity()))
		for i := 12; i >= 1; i-- {
			parent := cmd.Spawn(illusion.C(transform.FromXYZ(float32(i), 0, 0).WithRotation(rl.QuaternionFromAxisAngle(transform.Up, transform.Deg(90)))))
			parent.WithChild(illusion.C(SpotLight{Color: rl.White, Intensity: 2, Range: 50, Inner: 16, Outer: 28, Enabled: true}), illusion.C(transform.FromXYZ(0, 1, -1)))
		}
		// The closest light is off, and must not consume a slot.
		cmd.Spawn(illusion.C(SpotLight{Range: 50, Intensity: 2}), illusion.C(transform.Identity()))
	}))
	t.Cleanup(app.Cleanup)
	app.Tick(time.Second / 60)
	u := shader.Uniforms
	if u["spotCount"][0] != maxSpots {
		t.Fatalf("%v spotlights, want %d", u["spotCount"], maxSpots)
	}
	// Local -Z becomes world -X; the nearest parent at X=1 has a lens at X=0.
	p, d := u["spotPos[0]"], u["spotDir[0]"]
	if abs32(p[0]) > .001 || p[1] != 1 || abs32(p[2]) > .001 || d[0] > -.99 {
		t.Fatalf("light did not follow its parent's pose: position %v, direction %v", p, d)
	}
	if u["spotPos[7]"][0] > 7.01 {
		t.Fatal("distant lights displaced nearer lights")
	}
	if cone := u["spotCone[0]"]; cone[0] >= cone[1] {
		t.Fatalf("reversed cone edges: %v", cone)
	}
	// The driven vehicle's lamps remain selected even among nearer cars.
	for q := ecs.NewFilter2[SpotLight, transform.GlobalTransform](app.World).Query(); q.Next(); {
		l, tr := q.Get()
		if tr.Translation().X > 10 {
			l.Priority = 1
		}
	}
	app.Tick(time.Second / 60)
	if u["spotPos[0]"][0] < 10 {
		t.Fatal("nearby parked cars displaced a priority beam")
	}
	for q := ecs.NewFilter1[SpotLight](app.World).Query(); q.Next(); {
		q.Get().Enabled = false
	}
	app.Tick(time.Second / 60)
	if u["spotCount"][0] != 0 {
		t.Fatal("disabled lights left stale beams in the shader")
	}
}

func abs32(v float32) float32 {
	if v < 0 {
		return -v
	}
	return v
}
