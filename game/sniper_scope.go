package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/window"
	"math"
	"runtime"
)

type scopeHidden struct{}

// Keep the local body, clothing and held model out of the optical view while
// retaining their shadows. Only undo visibility tags this system introduced.
func scopePlayerVisibility(cmd *illusion.Commands, actors *combatActors, state *illusion.Res[combatState], hidden *illusion.Query1[scopeHidden], shadows *illusion.Query1[render.ShadowOnly]) {
	if state.Get().sniperScoped() {
		player, _, _, _, _, ok := actors.player()
		if !ok {
			return
		}
		actors.family.Get().EachDescendant(player, func(e ecs.Entity) {
			if !shadows.Contains(e) {
				cmd.Entity(e).Insert(illusion.C(scopeHidden{}), illusion.C(render.ShadowOnly{}))
			}
		})
	} else {
		hidden.Each(func(e ecs.Entity, _ *scopeHidden) {
			cmd.Entity(e).Remove(ecs.C[scopeHidden](), ecs.C[render.ShadowOnly]())
		})
	}
}

func scopeRadius(width, height float32) float32 { return min(width, height) * .385 }

// Composite a crisp optical render inside a softly shaded metal housing.
func drawSniperScope(state *illusion.Res[combatState], menu *illusion.Res[menu], win *illusion.Res[window.Window], scene *illusion.Res[scopeScene]) {
	s := state.Get()
	if menu.Get().screen() != playing || !s.sniperScoped() {
		return
	}
	opacity := s.scopeBlend()
	if opacity <= 0 {
		return
	}
	w, h := float32(win.Get().Width), float32(win.Get().Height)
	x, y, r := w/2, h/2, scopeRadius(w, h)
	tint := rl.NewColor(255, 255, 255, uint8(255*opacity))
	if scene.Get().ready {
		texture := scene.Get().target.Texture
		for row := y - r; row < y+r; row += 1 {
			height := min(float32(1), y+r-row)
			dy := row + height/2 - y
			half := float32(math.Sqrt(float64(max(0, r*r-dy*dy))))
			source := scopeLensSource(w, h, x-half, row, 2*half, height, float32(texture.Width))
			rl.DrawTexturePro(texture, source, rl.Rectangle{X: x - half, Y: row, Width: 2 * half, Height: height}, rl.Vector2{}, 0, tint)
		}
	}
	if scene.Get().housing.ID == 0 {
		scene.Get().housing = makeScopeHousing()
	}
	housing := scene.Get().housing
	extent := r / .88 * 1.08
	rl.DrawTexturePro(housing, rl.Rectangle{Width: float32(housing.Width), Height: float32(housing.Height)}, rl.Rectangle{X: x - extent, Y: y - extent, Width: extent * 2, Height: extent * 2}, rl.Vector2{}, 0, tint)
	ink := rl.NewColor(13, 17, 16, uint8(245*opacity))
	line := func(x1, y1, x2, y2, thick float32) {
		rl.DrawRectangleRec(rl.Rectangle{X: min(x1, x2) - thick/2, Y: min(y1, y2) - thick/2, Width: max(thick, float32(math.Abs(float64(x2-x1)))), Height: max(thick, float32(math.Abs(float64(y2-y1))))}, ink)
	}
	// Heavy outer posts draw the eye toward a fine, uncluttered aiming centre.
	for _, sign := range []float32{-1, 1} {
		line(x+sign*r*.97, y, x+sign*r*.46, y, 3)
		line(x+sign*r*.46, y, x+sign*4, y, 1)
	}
	line(x, y-r*.97, x, y-4, 1)
	line(x, y+4, x, y+r*.46, 1)
	line(x, y+r*.46, x, y+r*.97, 3)
	for i := 1; i <= 3; i++ {
		d := float32(i) * r * .115
		tick := float32(3)
		if i%2 == 0 {
			tick = 5
		}
		line(x-tick, y+d, x+tick, y+d, 1)
		line(x-d, y-3, x-d, y+3, 1)
		line(x+d, y-3, x+d, y+3, 1)
	}
	color := rl.NewColor(204, 62, 42, uint8(255*opacity))
	if s.HitTime > 0 {
		color = rl.NewColor(colAccent.R, colAccent.G, colAccent.B, uint8(255*opacity))
	}
	rl.DrawRectangleRec(rl.Rectangle{X: x - 1, Y: y - 1, Width: 2, Height: 2}, color)
}

// Generate an original antialiased housing once, with bevels, grip grooves and
// a narrow lens-edge vignette. The centre and the outside stay transparent.
func makeScopeHousing() rl.Texture2D {
	const size = 1024
	pixels := make([]byte, size*size*4)
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			px, py := (float64(x)+.5-size/2)/(size/2)*1.08, (float64(y)+.5-size/2)/(size/2)*1.08
			r := math.Hypot(px, py)
			if r < .79 || r > 1.07 {
				continue
			}
			a := math.Atan2(py, px)
			light := (1 + math.Cos(a+2.35)) / 2
			value, alpha := float64(12), float64(0)
			switch {
			case r < .88:
				alpha = math.Pow((r-.79)/.09, 3) * 150
			case r < .90:
				value = 18 + light*22
				alpha = 255
			case r < .917:
				value = 45 + light*60
				alpha = 255
			case r < .979:
				value = 24 + light*22 + 3*math.Cos(a*140)
				alpha = 255
			case r < 1:
				value = 38 + light*50
				alpha = 255 * math.Min(1, (1-r)/.002)
			default:
				alpha = 90 * math.Pow((1.07-r)/.07, 2)
			}
			i := (y*size + x) * 4
			pixels[i], pixels[i+1], pixels[i+2], pixels[i+3] = uint8(value*.94), uint8(value), uint8(value*1.02), uint8(alpha)
		}
	}
	texture := rl.LoadTextureFromImage(rl.NewImage(pixels, size, size, 1, rl.UncompressedR8g8b8a8))
	runtime.KeepAlive(pixels)
	rl.SetTextureFilter(texture, rl.FilterBilinear)
	return texture
}
