// Package shading is the game's look: toon shading for everything, and ink
// outlines for what asks for them.
package shading

import (
	"image/color"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
)

// Plugin draws the world with the toon shader. It goes after the default
// plugins.
type Plugin struct {
	// ShadowColor tints the ambient light in the shadows.
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
}

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
	app.InsertResource(illusion.R(&render.Shader{
		Fragment: toonFragment,
		Uniforms: map[string][]float32{
			"shadowColor":  rgb(pl.ShadowColor),
			"softness":     {pl.Softness},
			"midBand":      {pl.MidBand},
			"rimColor":     rgb(pl.RimColor),
			"rimPower":     {pl.RimPower},
			"rimThreshold": {pl.RimThreshold},
			"fogColor":     rgb(pl.FogColor),
			"fogDistance":  {pl.FogDistance},
			"fogEnd":       {pl.FogEnd},
		},
	}))
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
}

func rgb(c color.RGBA) []float32 {
	return []float32{float32(c.R) / 255, float32(c.G) / 255, float32(c.B) / 255}
}
