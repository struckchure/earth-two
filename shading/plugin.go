// Package shading is the game's look: toon shading for everything, and ink
// outlines for what asks for them.
package shading

import (
	"fmt"
	"image/color"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Plugin draws the world with the toon shader. It goes after the default
// plugins.
type Plugin struct {
	// ShadowColor tints the ambient light on shaded faces. Cast shadows
	// darken this fill towards black.
	ShadowColor color.RGBA
	// Softness is how wide the edges between the bands are, in the cosine
	// of the angle to the light; MidBand is where the half-lit band ends.
	Softness, MidBand float32
	// RimColor is the light on the edges of what's lit; a higher RimPower
	// and RimThreshold keep it nearer the edge.
	RimColor               color.RGBA
	RimPower, RimThreshold float32
	// OutlineColor tints an outline's surface, and OutlineWidth is its
	// width in pixels.
	OutlineColor color.RGBA
	OutlineWidth float32
	// Haze: what's far off fades to FogColor, a share 1 - 1/e of the way at
	// FogDistance (0 for no haze), and more with distance, up to FogEnd,
	// past which (the sky) none.
	FogColor            color.RGBA
	FogDistance, FogEnd float32
	// GroundFill is the light bounced up off the ground, at
	// GroundBrightness: what faces down gets in place of the ambient light,
	// which then comes from the sky above. Black for an ambient the same
	// from everywhere.
	GroundFill       color.RGBA
	GroundBrightness float32
	// Exposure scales the light (0 for 1), and colours brighter than Knee
	// roll off towards white rather than clipping, keeping their hue (0
	// for none).
	Exposure, Knee float32
	// Zones are places lit differently from the open air, at most
	// maxZones of them; where they overlap, the later wins.
	Zones []Zone
	// Lamps are fixed world lights. Their illumination depends on the
	// surface's position, never on which lights are nearest the camera.
	Lamps []Lamp
}

// A Zone is a box of the world, Min to Max, lit its own way: inside it
// the ambient light is Ambient at Brightness, and the sun is tinted Sun
// (white leaves it be, black puts it out), fading in over Blend metres
// from its faces. Shadows still fall in it, so a roof still keeps the sun
// off what's under it.
type Zone struct {
	Min, Max   rl.Vector3
	Blend      float32
	Ambient    color.RGBA
	Brightness float32
	Sun        color.RGBA
}

// Haze is a resource: the dust in the air as the shaders draw it, starting
// as the Plugin's haze. Change it and the shaders follow: a dust storm closes
// it in and turns it brown. Veil is how much of the dust hides what's past
// the haze's end (the sky, the sun and the planets): 0 in clear air.
type Haze struct {
	Color         color.RGBA
	Distance, End float32
	Veil          float32
}

// maxZones is how many zones the toon shader takes.
const maxZones = 4

// Smooth is the material colour that has the toon shader light something
// smoothly, with no bands: the ground, whose slopes in bands read as stains.
// It's white, and all but opaque, which is how the shader knows it.
var Smooth = color.RGBA{R: 255, G: 255, B: 255, A: 254}

// Outline is the shader of the outline pass.
var Outline = &render.Shader{Vertex: outlineVertex, Fragment: outlineFragment}

// BoxOutline is the outline shader for a render.Cuboid.
var BoxOutline = &render.Shader{Vertex: boxOutlineVertex, Fragment: outlineFragment}

// BoxOutlinePass draws an outline around a render.Cuboid.
func BoxOutlinePass() render.Pass {
	return render.Pass{Shader: BoxOutline, CullFront: true}
}

// OutlinePass draws an outline around a model, without the meshes in skip.
func OutlinePass(skip map[int]bool) render.Pass {
	return render.Pass{Shader: Outline, CullFront: true, Skip: skip}
}

// outlineReach is how far away, in metres, outlines start thinning with
// distance rather than swallowing what they're around.
const outlineReach = 7

func (pl Plugin) Build(app *illusion.App) {
	exposure := pl.Exposure
	if exposure == 0 {
		exposure = 1
	}
	hemisphere := float32(0)
	if pl.GroundFill != (color.RGBA{}) {
		hemisphere = 1
	}
	uniforms := map[string][]float32{
		"shadowColor":  rgb(pl.ShadowColor),
		"softness":     {pl.Softness},
		"midBand":      {pl.MidBand},
		"rimColor":     rgb(pl.RimColor),
		"rimPower":     {pl.RimPower},
		"rimThreshold": {pl.RimThreshold},
		"fogColor":     rgb(pl.FogColor),
		"fogDistance":  {pl.FogDistance},
		"fogEnd":       {pl.FogEnd},
		"groundFill":   scaled(pl.GroundFill, pl.GroundBrightness),
		"hemisphere":   {hemisphere},
		"exposure":     {exposure},
		"knee":         {pl.Knee},
		"zoneCount":    {float32(min(len(pl.Zones), maxZones))},
	}
	for i, z := range pl.Zones[:min(len(pl.Zones), maxZones)] {
		uniforms[fmt.Sprintf("zoneMin[%d]", i)] = []float32{z.Min.X, z.Min.Y, z.Min.Z}
		uniforms[fmt.Sprintf("zoneMax[%d]", i)] = []float32{z.Max.X, z.Max.Y, z.Max.Z}
		uniforms[fmt.Sprintf("zoneBlend[%d]", i)] = []float32{max(z.Blend, 0.01)}
		uniforms[fmt.Sprintf("zoneAmbient[%d]", i)] = scaled(z.Ambient, z.Brightness)
		uniforms[fmt.Sprintf("zoneSun[%d]", i)] = rgb(z.Sun)
	}
	uniforms["veil"] = []float32{0}
	uniforms["spotCount"] = []float32{0}
	toon := &render.Shader{Fragment: lightingGLSL + lampGLSL(pl.Lamps) + toonSurface, Uniforms: uniforms}
	app.InsertResource(illusion.R(toon))
	app.InsertResource(illusion.R(&Haze{Color: pl.FogColor, Distance: pl.FogDistance, End: pl.FogEnd}))
	app.AddSystems(illusion.PostUpdate, illusion.Fn3(gatherSpots).After(transform.Propagate))
	Outline.Uniforms = map[string][]float32{
		"outlineColor": rgb(pl.OutlineColor),
		"reach":        {outlineReach},
		"fogColor":     rgb(pl.FogColor),
		"fogDistance":  {pl.FogDistance},
		"fogEnd":       {pl.FogEnd},
	}
	BoxOutline.Uniforms = Outline.Uniforms
	app.AddSystems(illusion.Update, illusion.Fn0(func() {
		w, h := float32(rl.GetScreenWidth()), float32(rl.GetScreenHeight())
		if w > 0 && h > 0 {
			Outline.Uniforms["pixel"] = []float32{2 * pl.OutlineWidth / w, 2 * pl.OutlineWidth / h}
		}
	}))
	// The haze, as it is this frame, to both shaders.
	app.AddSystems(illusion.PostUpdate, illusion.Fn1(func(haze *illusion.Res[Haze]) {
		h := haze.Get()
		for _, u := range []map[string][]float32{toon.Uniforms, Outline.Uniforms} {
			u["fogColor"], u["fogDistance"], u["fogEnd"] = rgb(h.Color), []float32{h.Distance}, []float32{h.End}
		}
		toon.Uniforms["veil"] = []float32{h.Veil}
	}))
}

func rgb(c color.RGBA) []float32 {
	return []float32{float32(c.R) / 255, float32(c.G) / 255, float32(c.B) / 255}
}

// scaled is c at brightness b.
func scaled(c color.RGBA, b float32) []float32 {
	v := rgb(c)
	return []float32{v[0] * b, v[1] * b, v[2] * b}
}
