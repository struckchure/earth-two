package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/shading"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion/transform"
)

// fixedLamps reads the same light definitions and placements used by
// setup, including terrain height and rotation. The renderer compiles them
// into its shader before startup, so camera movement cannot switch lamps.
func fixedLamps(root string) ([]shading.Lamp, error) {
	k, err := world.Load(root, "world/world.json")
	if err != nil {
		return nil, err
	}
	placed, err := world.Layout(root, "world/landfall.json")
	if err != nil {
		return nil, err
	}
	var lamps []shading.Lamp
	for _, p := range placed {
		piece, ok := k.Pieces[p.Piece]
		if !ok {
			return nil, fmt.Errorf("world: no piece %q", p.Piece)
		}
		if len(piece.Lights) == 0 || piece.Vehicle != nil {
			continue
		}
		at := rl.Vector3{X: p.At[0], Y: p.At[1] + standOn(k, p), Z: p.At[2]}
		turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
		for _, l := range piece.Lights {
			light, position := l.In(at, turn)
			lamps = append(lamps, shading.Lamp{At: position, Light: light})
		}
	}
	return lamps, nil
}
