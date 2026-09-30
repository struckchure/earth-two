package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
)

// Plugin loads the roster and runs the character systems. It needs the
// default plugins and physics.Plugin.
type Plugin struct {
	Models []Model
}

func (pl Plugin) Build(app *illusion.App) {
	// PreStartup, so Startup systems can spawn characters.
	app.AddSystems(illusion.PreStartup, illusion.Fn3(func(
		cmd *illusion.Commands,
		models *asset.Loader[render.Model],
		anims *asset.Loader[render.Animations],
	) {
		r := &Roster{}
		for _, m := range pl.Models {
			s := Skin{Model: models.MustLoad(m.Path), Anims: anims.MustLoad(m.Path), Clips: m.Clips, Scale: m.Scale}
			if s.Scale == 0 {
				s.Scale = 1
				// The bind pose's height; models are rigged standing up.
				bb := rl.GetModelBoundingBox(models.Get(s.Model).Model)
				if h := bb.Max.Y - bb.Min.Y; h > 0 {
					s.Scale = bodyHeight / h
				}
			}
			r.Skins = append(r.Skins, s)
		}
		cmd.InsertResource(illusion.R(r))
	}))
	app.AddSystems(illusion.FixedUpdate, illusion.Fn1(locomote))
	app.AddSystems(illusion.Update, illusion.Chain(
		illusion.Fn2(playerInput),
		illusion.Fn5(changeSkin),
		illusion.Fn4(face),
		illusion.Fn6(animate),
	))
}
