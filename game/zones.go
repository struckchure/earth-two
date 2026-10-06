package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/shading"
)

// zoneWeight is how far into zone z the point p is, 0 to 1: the toon
// shader's blend (shading/toon.go), so what's heard and what's lit change
// in the same place.
func zoneWeight(z shading.Zone, p rl.Vector3) float32 {
	in := func(lo, hi, v float32) float32 {
		return smoothstep(lo, lo+z.Blend, v) * (1 - smoothstep(hi-z.Blend, hi, v))
	}
	return in(z.Min.X, z.Max.X, p.X) * in(z.Min.Y, z.Max.Y, p.Y) * in(z.Min.Z, z.Max.Z, p.Z)
}

// whereabouts is how much a point is in each of the light zones: the open air,
// under the dome, inside the Hull. They add up to 1.
type whereabouts struct {
	outside, dome, hull float32
}

// placesAt is how much p is outside, under the dome and in the Hull, the
// later zone in lightZones winning over the earlier, as in the shader.
func placesAt(p rl.Vector3) whereabouts {
	dome := zoneWeight(lightZones[0], p)
	hull := zoneWeight(lightZones[1], p)
	dome *= 1 - hull
	return whereabouts{outside: 1 - dome - hull, dome: dome, hull: hull}
}

// lightAt is the colour of the light at p, for what's drawn unlit there
// (the dust): the sun's outside, the zones' fill inside them.
func lightAt(p rl.Vector3) rl.Color {
	c := sunlight
	for _, z := range lightZones {
		c = mixColour(c, z.Ambient, zoneWeight(z, p))
	}
	return c
}
