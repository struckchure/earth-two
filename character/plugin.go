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
	// Wardrobe is the wardrobe file (see tools/makehuman/wardrobe.py),
	// relative to the asset root; "" means nothing to wear.
	Wardrobe string
}

func (pl Plugin) Build(app *illusion.App) {
	// PreStartup, so Startup systems can spawn characters.
	app.InsertResource(illusion.R(&Controls{Enabled: true}))
	app.AddSystems(illusion.PreStartup, illusion.Fn5(func(
		cmd *illusion.Commands,
		models *asset.Loader[render.Model],
		anims *asset.Loader[render.Animations],
		textures *asset.Loader[render.Texture],
		settings *illusion.Res[asset.Settings],
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

		w := &Wardrobe{Bodies: make([]BodyWardrobe, len(pl.Models))}
		if pl.Wardrobe != "" {
			s, _ := settings.TryGet()
			var err error
			if w, err = loadWardrobe(pl.Wardrobe, pl.Models, s, models, textures); err != nil {
				panic("character: " + err.Error())
			}
		}
		cmd.InsertResource(illusion.R(w))
	}))
	app.AddSystems(illusion.FixedUpdate, illusion.Fn4(locomote))
	app.AddSystems(illusion.Update, illusion.Chain(
		illusion.Fn3(playerInput),
		illusion.Fn8(dress),
		illusion.Fn6(clothe),
		illusion.Fn4(face),
		illusion.Fn6(animate),
	))
	app.AddSystems(illusion.PostUpdate, illusion.Fn3(mirrorPose).After(render.Animate))
}
