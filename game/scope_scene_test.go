package game

import (
	"math"
	"testing"
)

func TestOpticalProjectionKeepsThreeTimesMagnification(t *testing.T) {
	for _, screen := range [][2]float32{{1920, 1080}, {1280, 800}, {800, 1280}} {
		w, h := screen[0], screen[1]
		diameter := scopeRadius(w, h) * 2
		fov := scopeProjection(h, diameter, 45)
		// A point 2 degrees from the firing ray must remain on that ray's side,
		// with three times its normal screen displacement, in every aspect ratio.
		normal := float64(h) / 2 * math.Tan(2*math.Pi/180) / math.Tan(45*math.Pi/360)
		optic := float64(diameter) / 2 * math.Tan(2*math.Pi/180) / math.Tan(float64(fov)*math.Pi/360)
		if math.Abs(optic/normal-3) > .00001 {
			t.Fatalf("%vx%v magnification = %f", w, h, optic/normal)
		}
		size := float32(scopeResolution(diameter, 1))
		source := scopeLensSource(w, h, w/2-1, h/2-1, 2, 2, size)
		if math.Abs(float64(source.X+source.Width/2-size/2)) > .001 {
			t.Fatal("lens crop shifted the firing ray horizontally")
		}
		if math.Abs(float64(source.Y-source.Height/2-size/2)) > .001 {
			t.Fatal("lens crop shifted the firing ray vertically")
		}
	}
}
func TestScopeResolutionMatchesDisplayPixelsWithinBudget(t *testing.T) {
	if scopeResolution(900, 1) < 900 || scopeResolution(900, 2) < 1800 {
		t.Fatal("scope undersamples the display")
	}
	if scopeResolution(2000, 2) > 2048 {
		t.Fatal("scope exceeded the texture budget")
	}
}
