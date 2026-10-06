package game

import (
	"reflect"
	"testing"
)

func TestScatterIsTheSameEachTime(t *testing.T) {
	for _, c := range [][2]int{{0, 0}, {75, -12}, {-40, 300}, {-163, -162}} {
		if a, b := scatterIn(c[0], c[1]), scatterIn(c[0], c[1]); !reflect.DeepEqual(a, b) {
			t.Errorf("cell %v: %v, then %v", c, a, b)
		}
	}
}

func TestScatterKeepsOffTheLevel(t *testing.T) {
	// Along the caravan road and round Landfall, the Pads and the hold:
	// nothing on the roads or the seats' ground.
	for ci := -10; ci <= 250; ci += 3 {
		for cj := -60; cj <= 310; cj += 7 {
			for _, s := range scatterIn(ci, cj) {
				if d := levelDistance(s.at.X, s.at.Z); d < scatterClear {
					t.Fatalf("%s at %v, %v m from level ground, want at least %v", s.piece, s.at, d, scatterClear)
				}
			}
		}
	}
}

func TestScatterLeavesTheDunesClear(t *testing.T) {
	// Round the Pads, where dunes_test.go drives: nothing to run into.
	ci, cj := scatterCellOf(dunes.X, dunes.Z)
	for di := -8; di <= 8; di++ {
		for dj := -8; dj <= 8; dj++ {
			for _, s := range scatterIn(ci+di, cj+dj) {
				for _, k := range scatterKinds {
					if k.piece == s.piece && k.minS == k.maxS && noBodies(s.at.X, s.at.Z) {
						t.Fatalf("%s at %v, in the clear round the Pads", s.piece, s.at)
					}
				}
			}
		}
	}
}

func TestTheFringeIsScattered(t *testing.T) {
	// Out in the open between Landfall and the hold: the bigger part of a
	// square kilometre has something on it.
	ci, cj := scatterCellOf(-800, 5000)
	cells, filled := 0, 0
	for di := range 25 {
		for dj := range 25 {
			cells++
			if len(scatterIn(ci+di, cj+dj)) > 0 {
				filled++
			}
		}
	}
	if filled < cells/2 {
		t.Fatalf("%d of %d cells have anything on them, want most", filled, cells)
	}
}
