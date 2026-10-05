package game

import (
	"math"
	"testing"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestTheSunIsTRAPPIST1FromTheRed(t *testing.T) {
	// 0.1192 solar radii seen from 0.02925 AU: 2.2° across.
	if d := 2 * sunRadius() * 180 / math.Pi; d < 2.1 || d > 2.3 {
		t.Errorf("the sun is %.2f° across, want about 2.2°", d)
	}
	if up := math.Asin(float64(sunFrom.Y)) * 180 / math.Pi; up < 10 || up > 30 {
		t.Errorf("the sun is %.0f° up, want it low (10° to 30°)", up)
	}
}

func TestNeighboursAreOnTheEcliptic(t *testing.T) {
	s, v := ecliptic()
	normal := rl.Vector3CrossProduct(s, v)
	for _, n := range neighbours {
		for _, days := range []float64{0, 3.3, 11.7} {
			dir, _ := neighbourAt(n, days)
			if off := rl.Vector3DotProduct(dir, normal); math.Abs(float64(off)) > 1e-4 {
				t.Errorf("%s on day %v is %v off the ecliptic", n.name, days, off)
			}
		}
	}
}

func TestTheEclipticStaysLow(t *testing.T) {
	s, v := ecliptic()
	if d := rl.Vector3DotProduct(s, v); math.Abs(float64(d)) > 1e-5 {
		t.Errorf("v isn't a quarter round from the sun: %v", d)
	}
	var high float64
	for a := 0.0; a < 2*math.Pi; a += .01 {
		dir := rl.Vector3Add(rl.Vector3Scale(s, float32(math.Cos(a))), rl.Vector3Scale(v, float32(math.Sin(a))))
		high = max(high, math.Asin(float64(dir.Y))*180/math.Pi)
	}
	if want := eclipticTilt * 180 / math.Pi; math.Abs(high-want) > .5 {
		t.Errorf("the ecliptic rises to %.1f°, want %.1f°", high, want)
	}
}

func TestNeighboursSizes(t *testing.T) {
	for _, n := range neighbours {
		// Nearest, at conjunction on the same side of the sun, and
		// furthest, on the far side; drawn planetScale times their size.
		near := math.Abs(n.a-theRed.a) * auKm
		far := (n.a + theRed.a) * auKm
		lo := planetScale * math.Asin(n.r*earthKm/far)
		hi := planetScale * math.Asin(n.r*earthKm/near)
		for days := 0.0; days < 40; days += .5 {
			_, r := neighbourAt(n, days)
			if r < lo-1e-9 || r > hi+1e-9 {
				t.Fatalf("%s on day %v: radius %v outside %v..%v", n.name, days, r, lo, hi)
			}
		}
	}
	// f at its nearest is about the Moon's size (0.55° across), as drawn
	// planetScale times bigger.
	f := neighbours[3]
	if d := 2 * math.Asin(f.r*earthKm/(math.Abs(f.a-theRed.a)*auKm)) * 180 / math.Pi; d < .5 || d > .6 {
		t.Errorf("f at its nearest is %.2f° across, want about 0.55°", d)
	}
}

func TestNeighboursMove(t *testing.T) {
	for _, n := range neighbours {
		a, _ := neighbourAt(n, 0)
		b, _ := neighbourAt(n, 1)
		// They come round again in their synodic period, so a day moves
		// them 360°/that.
		synodic := 1 / math.Abs(1/n.p-1/theRed.p)
		if moved := math.Acos(float64(min(1, rl.Vector3DotProduct(a, b)))) * 180 / math.Pi; moved < .1 || moved > 360/synodic*3 {
			t.Errorf("%s moved %.2f° in a day, synodic period %.1f days", n.name, moved, synodic)
		}
	}
}

func TestGlareThinsOut(t *testing.T) {
	last := float32(2)
	for deg := 0.0; deg <= 90; deg += .5 {
		g := glare(deg * math.Pi / 180)
		if g > last+1e-6 {
			t.Fatalf("glare grows again %v° out: %v after %v", deg, g, last)
		}
		last = g
	}
	if g := glare(0); g < .9 {
		t.Errorf("glare at the sun %v, want it dense", g)
	}
	if g := glare(90 * math.Pi / 180); g > .1 {
		t.Errorf("glare a quarter of the sky away %v, want it all but gone", g)
	}
}
