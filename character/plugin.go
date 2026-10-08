package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Sets in the Update schedule, in the order they run.
const (
	// Input writes the player's Intent from the keyboard. Systems that take
	// a key press for themselves (getting into a vehicle with E, say) run
	// after it and clear what it asked for.
	Input illusion.SystemSet = "character.Input"
	// Act carries out what characters want: seats them, dresses them, turns
	// and animates them.
	Act illusion.SystemSet = "character.Act"
)

// Plugin loads the roster and runs the character systems. It needs the
// default plugins and physics.Plugin.
type Plugin struct {
	Models []Model
	// Wardrobe is the wardrobe file (see tools/makehuman/wardrobe.py),
	// relative to the asset root; "" means nothing to wear.
	Wardrobe string
	// Outline is the pass that draws an outline around a model, but for the
	// meshes in skip; nil for no outlines. Bodies and clothes get one.
	Outline func(skip map[int]bool) render.Pass
	// DisableCloth keeps clothes animated by their skeleton without the
	// per-vertex cloth solver, for platforms with a tight frame budget.
	DisableCloth bool
}

func (pl Plugin) Build(app *illusion.App) {
	// PreStartup, so Startup systems can spawn characters.
	app.InsertResource(illusion.R(&Controls{Enabled: true}), illusion.R(&View{}), illusion.R(&Family{}))
	// Each entity's children, once a frame, for the systems that look up a
	// character's body and clothes for every character (family.go).
	app.AddSystems(illusion.PreUpdate, illusion.Fn3(indexFamily))
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
		w.outline = pl.Outline
		cmd.InsertResource(illusion.R(w))
	}))
	app.AddSystems(illusion.FixedUpdate, illusion.Chain(illusion.Fn8(traverse), illusion.Fn4(locomote)))
	app.AddSystems(illusion.FixedPostUpdate, illusion.Fn1(rememberMotion).After(physics.Writeback))
	app.AddSystems(illusion.FixedPostUpdate,
		illusion.Fn4(fallIncapacitated).Before(physics.Prepare),
		illusion.Fn4(stepRagdolls).After(physics.Writeback),
	)
	app.ConfigureSets(illusion.Update, Act.After(Input))
	app.AddSystems(illusion.Update, illusion.Fn1(stopDowned).After(Input).Before(Act))
	app.AddSystems(illusion.Update,
		illusion.Fn4(playerInput).InSet(Input),
		illusion.Chain(
			illusion.Fn8(sit),
			illusion.Fn8(dress),
			illusion.Fn7(clothe).RunIf(illusion.Cond0(func() bool { return !pl.DisableCloth })),
			illusion.Fn5(face),
			illusion.Fn8(animate),
		).InSet(Act),
	)
	app.AddSystems(illusion.PostUpdate,
		illusion.Fn7(poseDowned).Before(render.Animate).Before(transform.Propagate),
		illusion.Fn4(presentMotion).Before(render.Animate).Before(transform.Propagate),
		illusion.Fn8(fitPoseToWorld).InSet(render.Animate).After(render.AdvanceAnimations).Before(render.AttachBones),
		illusion.Fn6(ride).InSet(render.Animate).After(render.AdvanceAnimations).Before(render.AttachBones),
		illusion.Fn2(lockstep).InSet(render.Animate).After(render.AdvanceAnimations).Before(render.AttachBones),
		illusion.Fn3(mirrorPose).After(render.Animate),
	)
}
