package vehicle

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// LightCycle lets the game tell vehicles when it is dark.
type LightCycle struct{ Night bool }

type headlampMode uint8

const (
	headlampsAutomatic headlampMode = iota
	headlampsOff
	headlampsOn
)

// Headlamp marks a lamp mounted on the vehicle's interpolated shell.
type Headlamp struct{ Radius float32 }

var headlampColor = rl.NewColor(255, 240, 204, 255)

// headlampMounts places lamps on the chassis' front, +Z. A narrow bike
// gets one central lamp; wider vehicles get a pair.
func (s *Spec) headlampMounts() []HeadlampSpec {
	if len(s.Headlamps) > 0 {
		return s.Headlamps
	}
	b := s.bounds()
	width := b.Max.X - b.Min.X
	y := b.Min.Y + min(0.55, (b.Max.Y-b.Min.Y)*0.4)
	front := rl.Vector3{X: (b.Min.X + b.Max.X) / 2, Y: max(0.6, y), Z: b.Max.Z + 0.08}
	if s.Handling == "bike" {
		front.Y = max(front.Y, 0.85)
		return []HeadlampSpec{{At: [3]float32{front.X, front.Y, front.Z}, Radius: .06}}
	}
	left, right := front, front
	left.X += width * 0.34
	right.X -= width * 0.34
	return []HeadlampSpec{{At: [3]float32{left.X, left.Y, left.Z}, Radius: .06}, {At: [3]float32{right.X, right.Y, right.Z}, Radius: .06}}
}

func spawnHeadlamps(c *illusion.ChildBuilder, spec *Spec) {
	for _, mount := range spec.headlampMounts() {
		at := vec(mount.At)
		// Forward is -Z in transform, but the vehicle's nose is +Z.
		pose := transform.FromTranslation(at).LookingAt(rl.Vector3Add(at, rl.Vector3{Y: -0.10, Z: 1}), transform.Up)
		c.Spawn(illusion.C(Headlamp{Radius: mount.Radius}), illusion.C(pose), illusion.C(shading.SpotLight{
			Color: headlampColor, Intensity: 5, Range: 55, Inner: 16, Outer: 28,
		}))
	}
}

func updateHeadlamps(
	cars *illusion.Query1[Drivable],
	lamps *illusion.Query2[Headlamp, shading.SpotLight],
	hier *illusion.Hierarchy,
	cycle *illusion.Res[LightCycle],
	keys *illusion.Res[input.Keys],
	controls *illusion.Res[character.Controls],
	driving *illusion.Res[Driving],
) {
	night, dr := cycle.Get().Night, driving.Get()
	cars.Each(func(e ecs.Entity, d *Drivable) {
		// A manual off applies to this drive. Explicit on remains powered
		// after leaving, until the player switches it off again.
		if d.Driver.IsZero() && d.lightsMode == headlampsOff {
			d.lightsMode = headlampsAutomatic
		}
		d.Headlamps = d.lightsMode == headlampsOn || (d.lightsMode == headlampsAutomatic && night && !d.Driver.IsZero())
		if e == dr.Vehicle {
			if controls.Get().Enabled && keys.Get().JustPressed(rl.KeyH) {
				d.Headlamps = !d.Headlamps
				d.lightsMode = headlampsOff
				if d.Headlamps {
					d.lightsMode = headlampsOn
				}
			}
			dr.Headlamps = d.Headlamps
		}
	})
	lamps.Each(func(e ecs.Entity, _ *Headlamp, light *shading.SpotLight) {
		light.Enabled = false
		light.Priority = 0
		shell, ok := hier.Parent(e)
		if !ok {
			return
		}
		root, ok := hier.Parent(shell)
		if !ok {
			return
		}
		if d, ok := cars.Get(root); ok {
			light.Enabled = d.Headlamps
			if root == dr.Vehicle {
				light.Priority = 1
			}
		}
	})
}

// The lenses glow when powered and remain visible as dark glass by day.
// Drawing them directly avoids adding GPU assets to headless simulations.
func drawHeadlamps(
	lamps *illusion.Query3[Headlamp, shading.SpotLight, transform.GlobalTransform],
	cameras *illusion.Query2[render.Camera3d, transform.GlobalTransform],
	win *illusion.Res[window.Window],
	hier *illusion.Hierarchy,
	hidden *illusion.Query1[render.Hidden],
	shadowOnly *illusion.Query1[render.ShadowOnly],
) {
	if _, ok := win.TryGet(); !ok {
		return
	}
	_, _, camera, ok := cameras.Single()
	if !ok {
		return
	}
	lamps.Each(func(e ecs.Entity, lamp *Headlamp, light *shading.SpotLight, pose *transform.GlobalTransform) {
		if shell, ok := hier.Parent(e); !ok || hidden.Contains(shell) || shadowOnly.Contains(shell) {
			return
		}
		at := pose.Translation()
		if rl.Vector3Distance(at, camera.Translation()) > 150 {
			return
		}
		colour := rl.NewColor(45, 45, 40, 255)
		if light.Enabled {
			colour = headlampColor
		}
		radius := lamp.Radius
		if radius <= 0 {
			radius = .06
		}
		rl.DrawSphere(rl.Vector3Subtract(at, rl.Vector3Scale(pose.Forward(), radius)), radius*1.3, rl.NewColor(25, 25, 23, 255))
		rl.DrawSphere(at, radius, colour)
	})
}
