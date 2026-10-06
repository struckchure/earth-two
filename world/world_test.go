package world

import (
	"encoding/binary"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/transform"
)

const assets = "../assets"

func kit(t *testing.T) *Kit {
	t.Helper()
	k, err := Load(assets, "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	return k
}

// meshBounds is the box round every POSITION accessor in a .glb: its
// pieces are one mesh at the origin, so that's the model's bounds.
func meshBounds(t *testing.T, path string) (lo, hi rl.Vector3) {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	n := binary.LittleEndian.Uint32(b[12:16])
	var doc struct {
		Meshes []struct {
			Primitives []struct {
				Attributes map[string]int `json:"attributes"`
			} `json:"primitives"`
		} `json:"meshes"`
		Accessors []struct {
			Min []float32 `json:"min"`
			Max []float32 `json:"max"`
		} `json:"accessors"`
	}
	if err := json.Unmarshal(b[20:20+n], &doc); err != nil {
		t.Fatal(err)
	}
	inf := float32(math.Inf(1))
	lo, hi = rl.Vector3{X: inf, Y: inf, Z: inf}, rl.Vector3{X: -inf, Y: -inf, Z: -inf}
	for _, m := range doc.Meshes {
		for _, p := range m.Primitives {
			a := doc.Accessors[p.Attributes["POSITION"]]
			lo = rl.Vector3Min(lo, rl.Vector3{X: a.Min[0], Y: a.Min[1], Z: a.Min[2]})
			hi = rl.Vector3Max(hi, rl.Vector3{X: a.Max[0], Y: a.Max[1], Z: a.Max[2]})
		}
	}
	return lo, hi
}

// triangles is how many triangles a .glb's meshes have.
func triangles(t *testing.T, path string) int {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	n := binary.LittleEndian.Uint32(b[12:16])
	var doc struct {
		Meshes []struct {
			Primitives []struct {
				Indices int `json:"indices"`
			} `json:"primitives"`
		} `json:"meshes"`
		Accessors []struct {
			Count int `json:"count"`
		} `json:"accessors"`
	}
	if err := json.Unmarshal(b[20:20+n], &doc); err != nil {
		t.Fatal(err)
	}
	tris := 0
	for _, m := range doc.Meshes {
		for _, p := range m.Primitives {
			tris += doc.Accessors[p.Indices].Count / 3
		}
	}
	return tris
}

// The pieces are drawn one draw call each, many at once and in the
// browser too, so each keeps to its budget (tools/world/kit.py's BUDGET by
// kind, or its own): tiled architecture lightest, set pieces heaviest.
func TestTriangleBudgets(t *testing.T) {
	for name, p := range kit(t).Pieces {
		if p.Budget == 0 || p.Budget > 150000 {
			t.Errorf("%s: a budget of %d triangles", name, p.Budget)
		}
		if n := triangles(t, filepath.Join(assets, p.Model)); n > p.Budget {
			t.Errorf("%s: %d triangles, over its budget of %d", name, n, p.Budget)
		}
	}
}

// raylib indexes a mesh's vertices with 16 bits, so a mesh with more than
// 65,535 of them comes out as garbage triangles; the finish splits big
// pieces into meshes under that (tools/world/finish.py's SPLIT).
func TestMeshesFitSixteenBitIndices(t *testing.T) {
	for name, p := range kit(t).Pieces {
		b, err := os.ReadFile(filepath.Join(assets, p.Model))
		if err != nil {
			t.Fatal(err)
		}
		n := binary.LittleEndian.Uint32(b[12:16])
		var doc struct {
			Meshes []struct {
				Primitives []struct {
					Attributes map[string]int `json:"attributes"`
				} `json:"primitives"`
			} `json:"meshes"`
			Accessors []struct {
				Count int `json:"count"`
			} `json:"accessors"`
		}
		if err := json.Unmarshal(b[20:20+n], &doc); err != nil {
			t.Fatal(err)
		}
		for _, m := range doc.Meshes {
			for _, prim := range m.Primitives {
				if c := doc.Accessors[prim.Attributes["POSITION"]].Count; c > 65535 {
					t.Errorf("%s: a mesh of %d vertices, over raylib's 65,535", name, c)
				}
			}
		}
	}
}

// colliderBounds is the box round a collider's corners.
func colliderBounds(c Collider) (lo, hi rl.Vector3) {
	center, rot := c.In(rl.Vector3{}, rl.QuaternionIdentity())
	inf := float32(math.Inf(1))
	lo, hi = rl.Vector3{X: inf, Y: inf, Z: inf}, rl.Vector3{X: -inf, Y: -inf, Z: -inf}
	for _, sx := range []float32{-1, 1} {
		for _, sy := range []float32{-1, 1} {
			for _, sz := range []float32{-1, 1} {
				corner := rl.Vector3{X: sx * c.Size[0] / 2, Y: sy * c.Size[1] / 2, Z: sz * c.Size[2] / 2}
				p := rl.Vector3Add(center, rl.Vector3RotateByQuaternion(corner, rot))
				lo, hi = rl.Vector3Min(lo, p), rl.Vector3Max(hi, p)
			}
		}
	}
	return lo, hi
}

// The colliders are what traversal climbs, so they should be where the
// model is: inside its bounds, give or take a stair's ramp running into
// the floor.
func TestCollidersInsideModels(t *testing.T) {
	const slack = 0.25
	for name, p := range kit(t).Pieces {
		lo, hi := meshBounds(t, filepath.Join(assets, p.Model))
		if hi.Y-lo.Y <= 0 {
			t.Errorf("%s: an empty model", name)
		}
		for i, c := range p.Colliders {
			clo, chi := colliderBounds(c)
			if clo.X < lo.X-slack || clo.Y < lo.Y-slack || clo.Z < lo.Z-slack ||
				chi.X > hi.X+slack || chi.Y > hi.Y+slack || chi.Z > hi.Z+slack {
				t.Errorf("%s: collider %d (%v to %v) outside the model (%v to %v)", name, i, clo, chi, lo, hi)
			}
		}
	}
}

// Traversal's sizes (character/traversal.go): what's vaulted is 0.35 to
// 1 m high, what's mantled up to 1.8 m, and a slide passes under 0.9 m.
func TestTraversalSizes(t *testing.T) {
	k := kit(t)
	top := func(name string) float32 {
		_, hi := colliderBounds(k.Pieces[name].Colliders[0])
		return hi.Y
	}
	for _, name := range []string{"crate", "drum", "railing"} {
		if h := top(name); h < .35 || h > 1 {
			t.Errorf("%s is %.2f m high; vaults are 0.35 to 1 m", name, h)
		}
	}
	if h := top("crate_tall"); h <= 1 || h > 1.8 {
		t.Errorf("crate_tall is %.2f m high; mantles are 1 to 1.8 m", h)
	}
	lo, _ := colliderBounds(k.Pieces["low_duct"].Colliders[0])
	if lo.Y <= .9 || lo.Y >= 1.8 {
		t.Errorf("low_duct's underside is %.2f m up; slides need it between 0.9 and 1.8 m", lo.Y)
	}
	l := k.Pieces["ladder"].Ladders
	if len(l) != 1 || l[0].Width != .64 {
		t.Fatalf("ladder's ladders: %+v", l)
	}
	if _, hi := colliderBounds(k.Pieces["catwalk"].Colliders[0]); math.Abs(float64(hi.Y-l[0].Top[1]+.04)) > .01 {
		t.Errorf("the ladder's top (%.2f m) isn't 4 cm over the catwalk (%.2f m)", l[0].Top[1], hi.Y)
	}
}

// Every piece says what it is and where it's from; what's carried has
// nothing to collide with, and what's big enough to walk into does.
func TestKindsAndCategories(t *testing.T) {
	kinds := map[string]bool{"kit": true, "prop": true, "item": true, "vehicle": true, "wheel": true}
	for name, p := range kit(t).Pieces {
		if !kinds[p.Kind] {
			t.Errorf("%s: kind %q", name, p.Kind)
		}
		if p.Category == "" {
			t.Errorf("%s: no category", name)
		}
		if p.Kind == "item" && len(p.Colliders) > 0 {
			t.Errorf("%s: an item with colliders", name)
		}
		if p.Kind == "vehicle" && len(p.Colliders) == 0 {
			t.Errorf("%s: a vehicle with no colliders", name)
		}
		for i, c := range p.Colliders {
			if c.Size[0] <= 0 || c.Size[1] <= 0 || c.Size[2] <= 0 {
				t.Errorf("%s: collider %d is %v", name, i, c.Size)
			}
		}
	}
}

func TestLayoutNamesPieces(t *testing.T) {
	k := kit(t)
	layout, err := Layout(assets, "world/hull_block.json")
	if err != nil {
		t.Fatal(err)
	}
	if len(layout) == 0 {
		t.Fatal("an empty layout")
	}
	for _, p := range layout {
		if _, ok := k.Pieces[p.Piece]; !ok {
			t.Errorf("the layout places %q, which isn't a piece", p.Piece)
		}
	}
}

// A quarter turn is anticlockwise seen from above: what was along +X goes
// along -Z, as Blender's turn about Z takes +X to +Y (the game's -Z).
func TestQuarterTurn(t *testing.T) {
	c := Collider{Center: [3]float32{1, .5, 0}, Size: [3]float32{2, 1, .5}, Rotation: [4]float32{0, 0, 0, 1}}
	turn := rl.QuaternionFromAxisAngle(transform.Up, math.Pi/2)
	center, _ := c.In(rl.Vector3{X: 10}, turn)
	if rl.Vector3Distance(center, rl.Vector3{X: 10, Y: .5, Z: -1}) > 1e-5 {
		t.Errorf("turned centre %v, want (10, 0.5, -1)", center)
	}
	l := Ladder{Facing: [3]float32{0, 0, -1}}.In(rl.Vector3{}, turn)
	if rl.Vector3Distance(l.Facing, rl.Vector3{X: -1}) > 1e-5 {
		t.Errorf("turned facing %v, want (-1, 0, 0)", l.Facing)
	}
}

// A light turns with its piece, and keeps its colour.
func TestLightIn(t *testing.T) {
	l := Light{At: [3]float32{1, 2.5, 0}, Color: [3]float32{1, .5, 0}, Intensity: 2, Range: 6}
	light, at := l.In(rl.Vector3{X: 10}, rl.QuaternionFromAxisAngle(transform.Up, math.Pi/2))
	if rl.Vector3Distance(at, rl.Vector3{X: 10, Y: 2.5, Z: -1}) > 1e-5 {
		t.Errorf("turned light at %v, want (10, 2.5, -1)", at)
	}
	if light.Color != rl.NewColor(255, 128, 0, 255) || light.Intensity != 2 || light.Range != 6 {
		t.Errorf("light %+v", light)
	}
}

// The lamps light what's round them, from within their models.
func TestLampsGiveLight(t *testing.T) {
	k := kit(t)
	for _, name := range []string{"pendant_lamp", "work_lamp", "street_lamp", "floodlight_tower", "pad_beacon", "status_light"} {
		if len(k.Pieces[name].Lights) == 0 {
			t.Errorf("%s gives no light", name)
		}
	}
	const slack = 0.5
	for name, p := range k.Pieces {
		if len(p.Lights) == 0 {
			continue
		}
		lo, hi := meshBounds(t, filepath.Join(assets, p.Model))
		for i, l := range p.Lights {
			if l.Range <= 0 || l.Intensity <= 0 {
				t.Errorf("%s: light %d reaches %v m at %v", name, i, l.Range, l.Intensity)
			}
			at := vec(l.At)
			if at.X < lo.X-slack || at.Y < lo.Y-slack || at.Z < lo.Z-slack || at.X > hi.X+slack || at.Y > hi.Y+slack || at.Z > hi.Z+slack {
				t.Errorf("%s: light %d at %v, outside the model (%v to %v)", name, i, at, lo, hi)
			}
		}
	}
}

// A vehicle that drives has its wheel pieces, a seat with somewhere to get
// out, and a chassis that clears the ground with its suspension right up
// (as the game's handlings have it: at most 0.3 m of bump).
func TestDrivableVehicles(t *testing.T) {
	k := kit(t)
	drivable := 0
	for name, p := range k.Pieces {
		v := p.Vehicle
		if v == nil {
			continue
		}
		drivable++
		if v.Handling == "" || len(v.Wheels) == 0 || len(v.Seats) == 0 || len(v.Chassis) == 0 {
			t.Errorf("%s: drives with handling %q, %d wheels, %d seats, %d chassis boxes", name, v.Handling,
				len(v.Wheels), len(v.Seats), len(v.Chassis))
			continue
		}
		for i, w := range v.Wheels {
			if wp, ok := k.Pieces[w.Piece]; !ok || wp.Kind != "wheel" {
				t.Errorf("%s: wheel %d is drawn with %q, which isn't a wheel", name, i, w.Piece)
			}
			if w.Radius <= 0 || math.Abs(float64(w.At[1]-w.Radius)) > 0.08 {
				t.Errorf("%s: wheel %d of radius %v has its centre %v m up: parked, it should touch the ground", name, i, w.Radius, w.At[1])
			}
		}
		for i, c := range v.Chassis {
			lo, _ := colliderBounds(Collider(c))
			if lo.Y < 0.3 {
				t.Errorf("%s: chassis box %d reaches %.2f m from the ground: the suspension bottoming out would ground it", name, i, lo.Y)
			}
		}
		for _, s := range v.Seats {
			if len(s.Exits) == 0 {
				t.Errorf("%s: a seat with no way out", name)
			}
		}
	}
	if drivable < 6 {
		t.Errorf("%d vehicles drive, want the buggy, bike, trike, rover and both haulers", drivable)
	}
}
