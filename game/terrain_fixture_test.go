package game

import (
	"encoding/json"
	"math"
	"os"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

// terrainFixture is the ground at fixed points, for the Rust client's
// terrain (crates/world) to agree with. Regenerate it after changing the
// terrain with
//
//	EARTH_TWO_WRITE_FIXTURE=1 go test ./game -run TestTerrainFixture
//
// and check both clients' tests still pass.
const terrainFixture = "../crates/world/fixtures/terrain.json"

type terrainPoint struct {
	At     [2]float32 `json:"at"`
	Ground float32    `json:"ground"`
	Drawn  float32    `json:"drawn"`
	Walk   float32    `json:"walk"`
	Colour [3]uint8   `json:"colour"`
	Noise  float32    `json:"noise"`
}

// terrainFixturePoints is where the fixture samples: the seats and roads,
// the drift and berm, the canyons and the rim, and a coarse grid.
func terrainFixturePoints() []rl.Vector2 {
	points := []rl.Vector2{
		{X: -16, Y: 4}, {X: -4, Y: -36}, {X: 0, Y: 60}, {X: arrival.X, Y: arrival.Z},
		padsAt, holdAt, havenAt, {X: 3900, Y: -120}, {X: -850, Y: 5400},
		{X: -1625, Y: 12023}, {X: -6000, Y: -6200},
		// Against the dome's walls, and the hold's berm.
		{X: 40, Y: 0}, {X: -60, Y: 20}, {X: 10, Y: 50}, {X: -1500, Y: 11820}, {X: -1320, Y: 12100},
		// Canyon country, and the mountains.
		{X: -5800, Y: -6900}, {X: -7100, Y: -6000}, {X: 15500, Y: 1200}, {X: -2000, Y: -15800},
		// Just off the level, where the edges wander.
		{X: 50, Y: 70}, {X: 9830, Y: -1740}, {X: 2210, Y: 300},
	}
	for z := float32(-15000); z <= 15000; z += 2500 {
		for x := float32(-15000); x <= 15000; x += 2500 {
			points = append(points, rl.Vector2{X: x + 123.4, Y: z - 56.7})
		}
	}
	return points
}

func sampleTerrainFixture() []terrainPoint {
	var out []terrainPoint
	for _, p := range terrainFixturePoints() {
		c := groundColour(p.X, p.Y)
		out = append(out, terrainPoint{
			At:     [2]float32{p.X, p.Y},
			Ground: groundHeight(p.X, p.Y),
			Drawn:  drawnHeight(p.X, p.Y),
			Walk:   walkHeight(p.X, p.Y),
			Colour: [3]uint8{c.R, c.G, c.B},
			Noise:  fbm(p.X/45, p.Y/45),
		})
	}
	return out
}

func TestTerrainFixture(t *testing.T) {
	got := sampleTerrainFixture()
	if os.Getenv("EARTH_TWO_WRITE_FIXTURE") != "" {
		data, err := json.MarshalIndent(got, "", " ")
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(terrainFixture, append(data, '\n'), 0o644); err != nil {
			t.Fatal(err)
		}
		return
	}
	data, err := os.ReadFile(terrainFixture)
	if err != nil {
		t.Fatal(err)
	}
	var want []terrainPoint
	if err := json.Unmarshal(data, &want); err != nil {
		t.Fatal(err)
	}
	if len(want) != len(got) {
		t.Fatalf("%d fixture points, want %d: regenerate it", len(want), len(got))
	}
	for i, w := range want {
		g := got[i]
		if g.At != w.At || math.Abs(float64(g.Ground-w.Ground)) > 1e-4 || math.Abs(float64(g.Drawn-w.Drawn)) > 1e-4 ||
			math.Abs(float64(g.Walk-w.Walk)) > 1e-4 || g.Colour != w.Colour || math.Abs(float64(g.Noise-w.Noise)) > 1e-6 {
			t.Errorf("at %v: %+v, the fixture has %+v: the terrain changed; regenerate it", g.At, g, w)
		}
	}
}
