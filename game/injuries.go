package game

import (
	"fmt"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

type recoveryActors struct {
	players illusion.Query2Where[character.Health, transform.Transform, illusion.With[character.Player]]
	cameras illusion.Query1Where[transform.Transform, illusion.With[render.Camera3d]]
}

func (r *recoveryActors) InitParam(w *ecs.World) { r.players.InitParam(w); r.cameras.InitParam(w) }

func recoverPlayer(keys *illusion.Res[input.Keys], mu *illusion.Res[menu], actors *recoveryActors, cmd *illusion.Commands, settings *illusion.Res[physics.Settings]) {
	if mu.Get().screen() != playing || settings.Get().Paused || !keys.Get().JustPressed(rl.KeyR) {
		return
	}
	e, health, at, ok := actors.players.Single()
	if !ok || health.State == character.Healthy {
		return
	}
	feet := rl.Vector3Add(arrival, rl.Vector3{Y: .05})
	delta := rl.Vector3Subtract(rl.Vector3Add(feet, rl.Vector3{Y: .9}), at.Translation)
	actors.cameras.Each(func(_ ecs.Entity, camera *transform.Transform) {
		camera.Translation = rl.Vector3Add(camera.Translation, delta)
	})
	character.Revive(cmd, e, feet)
	keys.Get().Clear() // recovery must not also ask for a roll
}

func drawInjury(win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], mu *illusion.Res[menu], players *illusion.Query1Where[character.Health, illusion.With[character.Player]]) {
	_, health, ok := players.Single()
	if !ok || health.State == character.Healthy || mu.Get().screen() != playing {
		return
	}
	w := win.Get()
	p := newPainter(fonts.Get(), w)
	width := p.px(380)
	r := rl.Rectangle{X: (float32(w.Width) - width) / 2, Y: float32(w.Height) * .3, Width: width, Height: p.px(116)}
	p.panel(r)
	title, action := "Critical condition", "Emergency recovery at the Pads"
	if health.State == character.Dead {
		title, action = "Dead", "Respawn at the Pads"
	}
	p.text(title, rl.Vector2{X: r.X + p.px(16), Y: r.Y + p.px(12)}, 24, semibold, colAccent)
	p.text(fmt.Sprintf("Vehicle impact · %.0f km/h", health.ImpactSpeed*3.6), rl.Vector2{X: r.X + p.px(16), Y: r.Y + p.px(48)}, 14, semibold, colText)
	x := r.X + p.px(16)
	x += p.keycap("R", rl.Vector2{X: x, Y: r.Y + p.px(76)}, 13) + p.px(8)
	p.text(action, rl.Vector2{X: x, Y: r.Y + p.px(80)}, 14, semibold, colText)
}

type injuryPlugin struct{}

func (injuryPlugin) Build(app *illusion.App) {
	app.AddSystems(illusion.Update, illusion.Fn5(recoverPlayer).Before(character.Input))
	app.AddSystems(illusion.Render, illusion.Fn4(drawInjury).InSet(render.Draw2D))
}
