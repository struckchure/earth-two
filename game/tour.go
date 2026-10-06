//go:build !js

package game

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// A View is a place the tour (see Tour) stops and takes a frame: the player
// stood at Player, and the camera orbiting them at Yaw and Pitch (as the
// mouse turns it in play), or, if Eye is set, there looking at Target. All
// in the game's frame (Y up), in metres. With Ground, the player stands on
// the terrain at Player's X and Z, whatever its Y. With Look, the player
// wears that faction's look (docs/look-and-feel.md), by name. With Use,
// the player presses E once they've landed (to sit on a bench in reach, say),
// and with Hold they hold a pose ("Talk", "Dance", "Fix"). Keep an Eye
// near its Player: the scene is still culled from where the orbit camera
// round the Player would be, so pieces vanish from an Eye far from it.
type View struct {
	Name   string      `json:"name"`
	Player [3]float32  `json:"player"`
	Ground bool        `json:"ground"`
	Yaw    float32     `json:"yaw"`
	Pitch  float32     `json:"pitch"`
	Eye    *[3]float32 `json:"eye,omitempty"`
	Target *[3]float32 `json:"target,omitempty"`
	Look   string      `json:"look,omitempty"`
	Use    bool        `json:"use,omitempty"`
	Hold   string      `json:"hold,omitempty"`
}

// tourHolds are the poses a View can Hold, by name.
var tourHolds = map[string]character.Anim{"Talk": character.Talk, "Dance": character.Dance, "Fix": character.Fix}

// tourSettle is how many frames the tour waits at a view before taking it:
// long enough for the terrain to stream in and the camera to catch up. A
// view that uses something or holds a pose does so at tourUseAt, once the
// player's surely landed, and is taken at tourUseSettle, once it's under
// way (sat down, knelt).
const (
	tourSettle    = 150
	tourUseAt     = 90
	tourUseSettle = 240
)

// Tour runs the game with no menus, takes a frame at each view in the JSON
// file viewsFile, saves it as <out>/<name>.png, and exits: the same places
// shot the same way, to compare the look before and after a change.
func Tour(viewsFile, out string) error {
	b, err := os.ReadFile(viewsFile)
	if err != nil {
		return err
	}
	var views []View
	if err := json.Unmarshal(b, &views); err != nil {
		return fmt.Errorf("%s: %w", viewsFile, err)
	}
	if err := os.MkdirAll(out, 0o755); err != nil {
		return err
	}
	at, frames := 0, 0
	app := build(&menu{})
	// After the camera's followed the player, and before transforms are
	// propagated for drawing.
	app.AddSystems(illusion.PostUpdate, illusion.Fn6(func(
		intents *illusion.Query1Where[character.Intent, illusion.With[character.Player]],
		players *illusion.Query2Where[transform.Transform, physics.CharacterController, illusion.With[character.Player]],
		cameras *illusion.Query2[transform.Transform, render.Camera3d],
		o *illusion.Res[orbit],
		outfits *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
		wardrobe *illusion.Res[character.Wardrobe],
	) {
		if at >= len(views) {
			return
		}
		v := views[at]
		p := rl.Vector3{X: v.Player[0], Y: v.Player[1], Z: v.Player[2]}
		if v.Ground {
			p.Y = groundHeight(p.X, p.Z) + groundLevel + 1
		}
		if v.Look != "" && frames == 0 {
			w := wardrobe.Get()
			outfits.Each(func(_ ecs.Entity, out *character.Outfit) {
				for i, l := range w.Bodies[out.Body].Looks {
					if l.Name == v.Look {
						*out = w.Wear(*out, i)
					}
				}
			})
		}
		if frames == tourUseAt {
			intents.Each(func(_ ecs.Entity, in *character.Intent) {
				if v.Use {
					in.Act = character.Interact
				}
				if hold, ok := tourHolds[v.Hold]; ok {
					in.Hold = hold
				}
			})
		}
		// Held there until they've landed, then left to stand.
		if frames < 40 {
			players.Each(func(_ ecs.Entity, tr *transform.Transform, cc *physics.CharacterController) {
				tr.Translation = p
				cc.Velocity = rl.Vector3{}
			})
		}
		orb := o.Get()
		orb.yaw, orb.pitch, orb.still = v.Yaw, v.Pitch, 0
		if v.Eye != nil && v.Target != nil {
			cameras.Each(func(_ ecs.Entity, tr *transform.Transform, _ *render.Camera3d) {
				tr.Translation = rl.Vector3{X: v.Eye[0], Y: v.Eye[1], Z: v.Eye[2]}
				tr.LookAt(rl.Vector3{X: v.Target[0], Y: v.Target[1], Z: v.Target[2]}, transform.Up)
			})
		}
	}).Before(transform.Propagate))
	app.AddSystems(illusion.Render, illusion.Fn0(func() {
		if at >= len(views) {
			os.Exit(0)
		}
		settle := tourSettle
		if views[at].Use || views[at].Hold != "" {
			settle = tourUseSettle
		}
		if frames++; frames < settle {
			return
		}
		img := rl.LoadImageFromScreen()
		rl.ExportImage(*img, filepath.Join(out, views[at].Name+".png"))
		rl.UnloadImage(img)
		at, frames = at+1, 0
	}).InSet(render.Draw2D))
	app.Run()
	return nil
}
