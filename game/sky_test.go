package game

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
)

func TestTheSunIsTRAPPIST1FromTheRed(t *testing.T) {
	// 0.1192 solar radii seen from 0.02925 AU: 2.2° across.
	if d := 2 * sunRadius() * 180 / math.Pi; d < 2.1 || d > 2.3 {
		t.Errorf("the sun is %.2f° across, want about 2.2°", d)
	}
}

func TestTheSunRisesAndSets(t *testing.T) {
	// Over a day: up from about 05:00 to 19:00, low even at noon, down in
	// the night; dusk's colour at the ends of the day, dark at midnight.
	day := time.Date(2026, 10, 6, 0, 0, 0, 0, time.UTC)
	at := func(h float64) rl.Vector3 { return sunAt(day.Add(time.Duration(h * float64(time.Hour)))) }
	upAt := func(h float64) float64 { return math.Asin(float64(at(h).Y)) * 180 / math.Pi }
	for _, h := range []float64{6, 9, 12, 15, 18} {
		if up := upAt(h); up <= 0 {
			t.Errorf("at %v:00 the sun's down (%.1f°), want it up", h, up)
		}
	}
	for _, h := range []float64{0, 2, 22} {
		if up := upAt(h); up >= 0 {
			t.Errorf("at %v:00 the sun's up (%.1f°), want night", h, up)
		}
	}
	if noon := upAt(12); noon > 32 {
		t.Errorf("at noon the sun's %.1f° up, want it low", noon)
	}
	if d := duskAt(at(12)); d > 0 {
		t.Errorf("at noon, %v of dusk's colour", d)
	}
	if d := duskAt(at(18.7)); d < .5 {
		t.Errorf("at 18:42, only %v of dusk's colour", d)
	}
	if n := nightAt(at(0)); n < 1 {
		t.Errorf("at midnight, only %v of the way into night", n)
	}
	if n := nightAt(at(12)); n > 0 {
		t.Errorf("at noon, %v of the way into night", n)
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

func TestTheStarsAreMostlyFaint(t *testing.T) {
	// Brightest first; most faint, a few bright; every colour there.
	if len(stars) != starCount+starBand {
		t.Fatalf("%d stars, want %d", len(stars), starCount+starBand)
	}
	bright, colours := 0, map[rl.Color]bool{}
	for i, s := range stars {
		if i > 0 && s.bright > stars[i-1].bright {
			t.Fatalf("star %d is brighter than the one before it", i)
		}
		if s.bright > .75 {
			bright++
		}
		colours[s.colour] = true
	}
	if bright < 10 || bright > len(stars)/20 {
		t.Errorf("%d bright stars of %d, want a few", bright, len(stars))
	}
	if len(colours) != len(starClasses) {
		t.Errorf("%d star colours, want %d", len(colours), len(starClasses))
	}
	if median := stars[len(stars)/2].bright; median > .35 {
		t.Errorf("the median star is %v bright, want most faint", median)
	}
}

func TestTheMilkyWayIsABand(t *testing.T) {
	// Bright along its plane, nothing at its poles.
	if b := milkyWay(galaxyCore); b < .2 {
		t.Errorf("at its core the Milky Way is %v bright", b)
	}
	if b := milkyWay(galaxyPole); b > .01 {
		t.Errorf("at its pole the Milky Way is %v bright, want none", b)
	}
}
