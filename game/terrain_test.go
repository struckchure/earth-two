package game

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
)

func TestTerrainSamplesMatchHeightAndNormal(t *testing.T) {
	calls := 0
	height := func(x, z float32) float32 {
		calls++
		return x*.2 - z*.3
	}
	const cells = 4
	samples := sampleTerrain(9800, -1800, 256, cells, height)
	if calls != (cells+3)*(cells+3) {
		t.Fatalf("evaluated grid %d times, want one height per point including the normal's border", calls)
	}
	for j := 0; j <= cells; j++ {
		for i := 0; i <= cells; i++ {
			x, z := float32(9800-128+i*64), float32(-1800-128+j*64)
			s := samples[j*(cells+1)+i]
			if s.height != height(x, z) {
				t.Fatalf("grid (%d,%d): height %v, want %v", i, j, s.height, height(x, z))
			}
			want := terrainNormal(height, x, z, 64)
			if rl.Vector3Distance(s.normal, want) > 1e-6 || math.Abs(float64(rl.Vector3Length(s.normal)-1)) > 1e-6 {
				t.Fatalf("grid (%d,%d): normal %v, want %v", i, j, s.normal, want)
			}
		}
	}
	// Check the real nonlinear terrain too, at the arrival and canyon country.
	for _, at := range []rl.Vector2{{X: arrival.X, Y: arrival.Z}, havenAt} {
		ci, cj := chunkOf(at.X, at.Y)
		center := chunkCentre(ci, cj)
		grid := sampleTerrain(center.X, center.Y, chunkSize, chunkCells, drawnHeight)
		for _, index := range [][2]int{{0, 0}, {chunkCells, chunkCells}, {chunkCells / 2, chunkCells / 2}, {7, 19}} {
			x := center.X - chunkSize/2 + float32(index[0])*chunkSize/chunkCells
			z := center.Y - chunkSize/2 + float32(index[1])*chunkSize/chunkCells
			s := grid[index[1]*(chunkCells+1)+index[0]]
			if s.height != drawnHeight(x, z) || rl.Vector3Distance(s.normal, terrainNormal(drawnHeight, x, z, chunkSize/chunkCells)) > 1e-6 {
				t.Fatalf("cached terrain differs at %v,%v", x, z)
			}
		}
	}
}

func BenchmarkTerrainSampling(b *testing.B) {
	b.Run("per_triangle_vertex", func(b *testing.B) {
		for i := 0; i < b.N; i++ {
			// GenMeshPlane emits six vertices per cell, including duplicates.
			for z := range chunkCells {
				for x := range chunkCells {
					for _, v := range [6][2]int{{x, z}, {x, z + 1}, {x + 1, z}, {x + 1, z}, {x, z + 1}, {x + 1, z + 1}} {
						wx, wz := float32(10000-128+v[0]*4), float32(-1600-128+v[1]*4)
						drawnHeight(wx, wz)
						terrainNormal(drawnHeight, wx, wz, 4)
					}
				}
			}
		}
	})
	b.Run("shared_grid", func(b *testing.B) {
		for i := 0; i < b.N; i++ {
			sampleTerrain(10000, -1600, chunkSize, chunkCells, drawnHeight)
		}
	})
}

func terrainTileVertices(cells int) []float32 {
	points, indices := grid(0, 0, tileSize, cells, func(float32, float32) float32 { return 0 })
	vertices := make([]float32, 0, len(indices)*3)
	for _, index := range indices {
		p := points[index]
		vertices = append(vertices, p.X, p.Y, p.Z)
	}
	return vertices
}

func tileHeights(samples []terrainSample) []float32 {
	heights := make([]float32, len(samples))
	for i, sample := range samples {
		heights[i] = sample.height
	}
	return heights
}

func TestCachedTileMaskMatchesTerrain(t *testing.T) {
	const cells = 64
	cx, cz := float32(9216), float32(-1024)
	vertices := terrainTileVertices(cells)
	heights := tileHeights(sampleTerrain(cx, cz, tileSize, cells, drawnHeight))
	for _, window := range []rl.Rectangle{detailWindow(35, -4), detailWindow(39, -8), detailWindow(0, 0)} {
		sink := sinkUnder(window)
		writeTileHeights(vertices, cx, cz, tileSize, cells, heights, sink)
		for i := 0; i < len(vertices); i += 3 {
			x, z := cx+vertices[i], cz+vertices[i+2]
			want := groundLevel + drawnHeight(x, z) - sink(x, z)
			if vertices[i+1] != want {
				t.Fatalf("tile masking changed height at %v,%v: %v vs %v", x, z, vertices[i+1], want)
			}
		}
	}
}

func BenchmarkTerrainTileMask(b *testing.B) {
	const cells = 64
	cx, cz := float32(9216), float32(-1024)
	vertices := terrainTileVertices(cells)
	heights := tileHeights(sampleTerrain(cx, cz, tileSize, cells, drawnHeight))
	sink := sinkUnder(detailWindow(35, -4))
	b.Run("rebuild", func(b *testing.B) {
		for i := 0; i < b.N; i++ {
			samples := sampleTerrain(cx, cz, tileSize, cells, drawnHeight)
			writeTileHeights(vertices, cx, cz, tileSize, cells, tileHeights(samples), sink)
		}
	})
	b.Run("cached", func(b *testing.B) {
		for i := 0; i < b.N; i++ {
			writeTileHeights(vertices, cx, cz, tileSize, cells, heights, sink)
		}
	})
}

func TestTerrainIsLevelWhereItMatters(t *testing.T) {
	for _, tt := range []struct {
		name string
		x, z float32
	}{
		{"the Hull", -16, 4},
		{"Charter Row", -4, -36},
		{"outside the gate", 0, 60},
		{"the arrival", arrival.X, arrival.Z},
		{"the Pads", padsAt.X, padsAt.Y},
		{"the caravan road", 3900, -120},
		{"the Fringe track", -850, 5400},
		{"the hold", holdAt.X, holdAt.Y},
		{"the farms", -1625, 12023},
		{"the salvage fields", -1375, 12020},
		{"the wind farm", -1600, 11900},
		{"the haven", havenAt.X, havenAt.Y},
		{"the wash out of the haven", -6000, -6200},
	} {
		if h := groundHeight(tt.x, tt.z); h != 0 {
			t.Errorf("%s: ground at %v, want level (0)", tt.name, h)
		}
	}
}

func TestTerrainRisesToTheEdge(t *testing.T) {
	var high float32
	for _, p := range []rl.Vector2{{X: 16000, Y: 0}, {X: -16000, Y: 3000}, {X: 2000, Y: -16000}, {X: -6000, Y: 16000}} {
		h := groundHeight(p.X, p.Y)
		high = max(high, h)
		if h < 80 {
			t.Errorf("at the world's edge %v, ground %v, want mountains", p, h)
		}
	}
	if high > 200 {
		t.Errorf("mountains %v m high, want under 200", high)
	}
}

func TestSeatsAreFarApart(t *testing.T) {
	seats := map[string]rl.Vector2{"Landfall": {}, "the Pads": padsAt, "the hold": holdAt, "the haven": havenAt}
	for a, pa := range seats {
		for b, pb := range seats {
			if a < b {
				if d := rl.Vector2Distance(pa, pb); d < 8000 {
					t.Errorf("%s and %s are %.0f m apart, want 8 km or more", a, b, d)
				}
			}
		}
	}
	for name, p := range map[string]rl.Vector2{"the Pads": padsAt, "the hold": holdAt, "the haven": havenAt} {
		if d := rl.Vector2Length(p); d > 15000 {
			t.Errorf("%s is %.0f m from Landfall, want 15 km at most", name, d)
		}
	}
}

func TestRocksAreOffTheRoads(t *testing.T) {
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, p := range placed {
		switch p.Piece {
		case "rock_small", "rock_large", "rock_spire", "dust_mound", "dry_brush", "dead_tree", "quiver_tree":
			if d, _ := roadDistance(p.At[0], p.At[2]); d < 9 {
				t.Errorf("%s at %v is %.1f m off a road: tools/world/landfall.py's ROADS and terrain.go's roads differ?", p.Piece, p.At, d)
			}
		}
	}
}

func TestPiecesStandOnTheGround(t *testing.T) {
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	lifted := 0
	for _, p := range placed {
		h := standOn(k, p)
		if under := groundHeight(p.At[0], p.At[2]); h > under+1e-4 {
			t.Errorf("%s at %v stands at %v, above the ground under its middle (%v)", p.Piece, p.At, h, under)
		}
		if h != 0 {
			lifted++
		}
	}
	if lifted == 0 {
		t.Error("nothing out in the Fringe stands off the level")
	}
}

func TestWalkingOffTheRoadOntoTheDunes(t *testing.T) {
	// On the caravan road, east of Landfall, walking north off it into
	// the dunes.
	l := newLandfall(t, rl.Vector3{X: 1000, Z: 167})
	l.tick(10)
	l.in.Move = rl.Vector3{Z: -1}
	l.tick(12 * 60)
	f := l.feet()
	if want := groundHeight(f.X, f.Z) + groundLevel; !l.cc.Grounded || f.Y < want-.3 || f.Y > want+.3 {
		t.Fatalf("out on the dunes at %v (grounded %v), want on the ground at %v", f, l.cc.Grounded, want)
	}
	if f.Z > 152 {
		t.Fatalf("walking north off the road, only got to z %v", f.Z)
	}
}
