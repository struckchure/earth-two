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

// The optic is a second, narrow-FOV scene render at the lens's pixel size.
// The ordinary scene stays on screen at normal FOV for peripheral awareness.
type scopeScene struct {
	target  rl.RenderTexture2D
	housing rl.Texture2D
	ready   bool
	saved   map[ecs.Entity]float32
}

const scopeScenePass illusion.ScheduleName = "game.ScopeScene"

func (s *scopeScene) free() {

	if s.target.Texture.ID != 0 {
		rl.UnloadTexture(s.target.Texture)
	}
	if s.target.Depth.ID != 0 {
		rl.UnloadTexture(s.target.Depth)
	}
	if s.target.ID != 0 {
		rl.UnloadFramebuffer(s.target.ID)
	}
	s.target = rl.RenderTexture2D{}
	if s.housing.ID != 0 {
		rl.UnloadTexture(s.housing)
		s.housing = rl.Texture2D{}
	}
}
func (s *scopeScene) resize(w, h int32) bool {
	if s.target.ID != 0 && s.target.Texture.Width == w && s.target.Texture.Height == h {
		return true
	}
	s.free()
	if w <= 0 || h <= 0 {
		return false
	}
	pixels := make([]byte, int(w)*int(h)*4)
	color := rl.LoadTextureFromImage(rl.NewImage(pixels, w, h, 1, rl.UncompressedR8g8b8a8))
	runtime.KeepAlive(pixels)
	depth := rl.Texture2D{ID: rl.LoadTextureDepth(w, h, false), Width: w, Height: h, Mipmaps: 1}
	id := rl.LoadFramebuffer()
	s.target = rl.RenderTexture2D{ID: id, Texture: color, Depth: depth}
	rl.FramebufferAttach(id, color.ID, 0, 100, 0)
	rl.FramebufferAttach(id, depth.ID, 100, 100, 0)
	if id == 0 || !rl.FramebufferComplete(id) {
		s.free()
		return false
	}
	rl.SetTextureFilter(color, rl.FilterBilinear)
	return true
}

// Match the old 3x angular magnification, but render the geometry with this
// projection instead of scaling pixels. The target is square, as is the lens.
func scopeProjection(height, diameter, baseFOV float32) float32 {
	return float32(2 * math.Atan(math.Tan(float64(baseFOV)*math.Pi/360)*float64(diameter/height)/3) * 180 / math.Pi)
}
func scopeResolution(diameter, scale float32) int32 {
	return int32(min(float32(2048), max(float32(256), float32(math.Ceil(float64(diameter*max(1, scale)))))))
}
func scopeRenderer(app *illusion.App) *illusion.SystemConfig {
	return illusion.Fn7(func(scene *illusion.Res[scopeScene], state *illusion.Res[combatState], menu *illusion.Res[menu], win *illusion.Res[window.Window], clear *illusion.Res[render.ClearColor], cameras *illusion.Query1[render.Camera3d], view *illusion.Res[render.View3D]) {
		s := scene.Get()
		s.ready = false
		if menu.Get().screen() != playing || !state.Get().sniperScoped() {
			return
		}
		w, h := float32(win.Get().Width), float32(win.Get().Height)
		if w <= 0 || h <= 0 {
			return
		}
		diameter := scopeRadius(w, h) * 2
		size := scopeResolution(diameter, drawScale())
		if !s.resize(size, size) {
			return
		}
		if s.saved == nil {
			s.saved = map[ecs.Entity]float32{}
		}
		clearSaved := s.saved
		for e := range clearSaved {
			delete(clearSaved, e)
		}
		cameras.Each(func(e ecs.Entity, camera *render.Camera3d) {
			s.saved[e] = camera.Fovy
			base := camera.Fovy
			if base == 0 {
				base = 45
			}
			camera.Fovy = scopeProjection(h, diameter, base)
		})
		mainView := *view.Get()
		rl.BeginTextureMode(s.target)
		rl.ClearBackground(clear.Get().Color)
		app.RunSchedule(scopeScenePass)
		rl.EndTextureMode()
		cameras.Each(func(e ecs.Entity, camera *render.Camera3d) {
			if fov, ok := s.saved[e]; ok {
				camera.Fovy = fov
			}
		})
		*view.Get() = mainView
		s.ready = true
	})
}
func scopeLensSource(w, h, x, y, width, height, size float32) rl.Rectangle {
	diameter := scopeRadius(w, h) * 2
	scale := size / diameter
	sx, sy := (x-(w-diameter)/2)*scale, (y-(h-diameter)/2)*scale
	return rl.Rectangle{X: sx, Y: size - sy - height*scale, Width: width * scale, Height: -height * scale}
}
func (combatPlugin) Cleanup(app *illusion.App) {
	if scene := ecs.GetResource[scopeScene](app.World); scene != nil {
		scene.free()
	}
}
