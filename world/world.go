// Package world places the pieces the world is built from: the Hull kit and
// the props that tools/world builds into assets/world (make world). Each
// piece is a model and the boxes it collides with, and maybe ladders; a
// layout file says where pieces go.
package world

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Plugin loads the pieces in Manifest at PreStartup, as the Kit resource,
// so Startup systems can place them. It needs the default plugins.
type Plugin struct {
	// Manifest is the pieces file (see tools/world/build.py), relative to
	// the asset root.
	Manifest string
	// Outline is the pass that draws an outline around a model; nil for none.
	Outline func(skip map[int]bool) render.Pass
}

func (pl Plugin) Build(app *illusion.App) {
	app.AddSystems(illusion.PreStartup, illusion.Fn3(func(
		cmd *illusion.Commands,
		models *asset.Loader[render.Model],
		settings *illusion.Res[asset.Settings],
	) {
		root := "assets"
		if s, ok := settings.TryGet(); ok && s.Root != "" {
			root = s.Root
		}
		k, err := Load(root, pl.Manifest)
		if err != nil {
			panic("world: " + err.Error())
		}
		for name, p := range k.Pieces {
			p.model = models.MustLoad(p.Model)
			k.Pieces[name] = p
		}
		k.outline = pl.Outline
		cmd.InsertResource(illusion.R(k))
	}))
}

// Kit is a resource with the pieces that can be placed.
type Kit struct {
	Pieces  map[string]Piece `json:"pieces"`
	root    string
	outline func(skip map[int]bool) render.Pass
}

// Piece is one piece of the world (architecture, a prop, a carried item or
// a vehicle), made standing on its origin in metres. A lamp has the light
// it gives.
type Piece struct {
	Model     string     `json:"model"`    // relative to the asset root
	Kind      string     `json:"kind"`     // "kit", "prop", "item", "vehicle" or "wheel" (a vehicle's)
	Category  string     `json:"category"` // the part of the world it's from, e.g. "The Fringe"
	Budget    int        `json:"budget"`   // the most triangles it may have
	Colliders []Collider `json:"colliders"`
	Ladders   []Ladder   `json:"ladders"`
	Lights    []Light    `json:"lights"`
	// Vehicle is how it drives, for a vehicle that can be driven.
	Vehicle *vehicle.Spec `json:"vehicle"`
	model   asset.Handle[render.Model]
}

// Collider is a box the piece collides with: its middle, its full size
// along its own axes and how it's turned.
type Collider struct {
	Center   [3]float32 `json:"center"`
	Size     [3]float32 `json:"size"`
	Rotation [4]float32 `json:"rotation"` // x, y, z, w
}

// Ladder is a character.Ladder in the piece's own frame.
type Ladder struct {
	Bottom     [3]float32 `json:"bottom"`
	Top        [3]float32 `json:"top"`
	Facing     [3]float32 `json:"facing"`
	BottomExit [3]float32 `json:"bottomExit"`
	TopExit    [3]float32 `json:"topExit"`
	Width      float32    `json:"width"`
}

// Light is a render.PointLight in the piece's own frame: where it is, its
// colour (0 to 1), how bright, and how far it reaches in metres.
type Light struct {
	At        [3]float32 `json:"at"`
	Color     [3]float32 `json:"color"`
	Intensity float32    `json:"intensity"`
	Range     float32    `json:"range"`
}

// In is the light, and where it is, on a piece at at turned by turn.
func (l Light) In(at rl.Vector3, turn rl.Quaternion) (render.PointLight, rl.Vector3) {
	c := func(v float32) uint8 { return uint8(math.Round(float64(max(0, min(1, v)) * 255))) }
	light := render.PointLight{
		Color:     rl.NewColor(c(l.Color[0]), c(l.Color[1]), c(l.Color[2]), 255),
		Intensity: l.Intensity,
		Range:     l.Range,
	}
	return light, rl.Vector3Add(at, rl.Vector3RotateByQuaternion(vec(l.At), turn))
}

// Placement is where a piece goes: at its origin's position, turned turns
// quarter turns anticlockwise seen from above.
type Placement struct {
	Piece string     `json:"piece"`
	At    [3]float32 `json:"at"`
	Turns int        `json:"turns"`
}

// Load reads the pieces file at path under root, without loading models.
func Load(root, path string) (*Kit, error) {
	k := &Kit{root: root}
	if err := readJSON(filepath.Join(root, path), k); err != nil {
		return nil, err
	}
	return k, nil
}

// Layout reads a layout file (e.g. hull_block.json) under root.
func Layout(root, path string) ([]Placement, error) {
	var f struct {
		Pieces []Placement `json:"pieces"`
	}
	err := readJSON(filepath.Join(root, path), &f)
	return f.Pieces, err
}

func readJSON(path string, v any) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, v); err != nil {
		return fmt.Errorf("%s: %w", path, err)
	}
	return nil
}

// SpawnLayout places every piece in the layout file at path, under the
// asset root.
func (k *Kit) SpawnLayout(cmd *illusion.Commands, path string) error {
	layout, err := Layout(k.root, path)
	if err != nil {
		return err
	}
	for _, p := range layout {
		if err := k.Place(cmd, p); err != nil {
			return err
		}
	}
	return nil
}

// Model is a piece's model, as loaded.
func (k *Kit) Model(piece string) asset.Handle[render.Model] { return k.Pieces[piece].model }

// look is how a model's drawn: the model and its outline.
func (k *Kit) look(model asset.Handle[render.Model]) []illusion.Component {
	out := []illusion.Component{illusion.C(render.Model3d{Model: model})}
	if k.outline != nil {
		out = append(out, illusion.C(render.Passes{k.outline(nil)}))
	}
	return out
}

// Place spawns a piece: its model, a static body for each of its colliders,
// its ladders and its lights; or, for a vehicle that drives, a vehicle
// parked there (see vehicle.Spawn).
func (k *Kit) Place(cmd *illusion.Commands, p Placement) error {
	piece, ok := k.Pieces[p.Piece]
	if !ok {
		return fmt.Errorf("world: no piece %q", p.Piece)
	}
	at := vec(p.At)
	turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
	if piece.Vehicle != nil {
		_, err := vehicle.Spawn(cmd, p.Piece, piece.Vehicle, piece.model, k.Model, k.look, at, turn)
		return err
	}
	model := []illusion.Component{
		illusion.C(render.Model3d{Model: piece.model}),
		illusion.C(transform.FromTranslation(at).WithRotation(turn)),
	}
	if k.outline != nil {
		model = append(model, illusion.C(render.Passes{k.outline(nil)}))
	}
	cmd.Spawn(model...)
	for _, c := range piece.Colliders {
		center, rot := c.In(at, turn)
		cmd.Spawn(
			illusion.C(transform.FromTranslation(center).WithRotation(rot)),
			illusion.C(physics.Static),
			illusion.C(physics.Cuboid(c.Size[0], c.Size[1], c.Size[2])),
		)
	}
	for _, l := range piece.Ladders {
		cmd.Spawn(illusion.C(l.In(at, turn)))
	}
	for _, l := range piece.Lights {
		light, where := l.In(at, turn)
		cmd.Spawn(illusion.C(light), illusion.C(transform.FromTranslation(where)))
	}
	return nil
}

// In is where the collider is, and how it's turned, on a piece at at turned
// by turn.
func (c Collider) In(at rl.Vector3, turn rl.Quaternion) (rl.Vector3, rl.Quaternion) {
	q := rl.Quaternion{X: c.Rotation[0], Y: c.Rotation[1], Z: c.Rotation[2], W: c.Rotation[3]}
	return rl.Vector3Add(at, rl.Vector3RotateByQuaternion(vec(c.Center), turn)), rl.QuaternionMultiply(turn, q)
}

// In is the ladder on a piece at at turned by turn.
func (l Ladder) In(at rl.Vector3, turn rl.Quaternion) character.Ladder {
	point := func(v [3]float32) rl.Vector3 { return rl.Vector3Add(at, rl.Vector3RotateByQuaternion(vec(v), turn)) }
	return character.Ladder{
		Bottom: point(l.Bottom), Top: point(l.Top),
		Facing:     rl.Vector3RotateByQuaternion(vec(l.Facing), turn),
		BottomExit: point(l.BottomExit), TopExit: point(l.TopExit),
		Width: l.Width,
	}
}

func vec(v [3]float32) rl.Vector3 { return rl.Vector3{X: v[0], Y: v[1], Z: v[2]} }
