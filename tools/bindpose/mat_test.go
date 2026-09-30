package main

import (
	"math"
	"testing"
)

func TestInverse(t *testing.T) {
	// A rotation of 50° about a tilted axis, a non-uniform scale, a move.
	ax := normalize([3]float64{1, 2, -0.5})
	s, c := math.Sin(0.4363), math.Cos(0.4363)
	m := local(obj{
		"translation": []any{0.3, -1.2, 4.0},
		"rotation":    []any{ax[0] * s, ax[1] * s, ax[2] * s, c},
		"scale":       []any{0.1, 2.0, 0.5},
	})
	got := m.mul(m.inverse())
	want := identity()
	for i := range got {
		if math.Abs(got[i]-want[i]) > 1e-9 {
			t.Fatalf("m * inverse(m) = %v, want identity", got)
		}
	}
}

func TestLocalTranslatesThenRotates(t *testing.T) {
	// 90° about Y: +X goes to -Z.
	h := math.Sqrt(0.5)
	m := local(obj{"translation": []any{1.0, 2.0, 3.0}, "rotation": []any{0.0, h, 0.0, h}})
	p := m.point([3]float64{1, 0, 0})
	want := [3]float64{1, 2, 2}
	for i := range p {
		if math.Abs(p[i]-want[i]) > 1e-9 {
			t.Fatalf("point = %v, want %v", p, want)
		}
	}
}

func TestDecomposeRoundTrip(t *testing.T) {
	ax := normalize([3]float64{-0.3, 1, 0.8})
	s, c := math.Sin(1.1), math.Cos(1.1)
	in := obj{
		"translation": []any{0.5, 2.0, -3.0},
		"rotation":    []any{ax[0] * s, ax[1] * s, ax[2] * s, c},
		"scale":       []any{100.0, 100.0, 100.0},
	}
	m := local(in)
	tr, r, sc := m.decompose()
	back := local(obj{
		"translation": []any{tr[0], tr[1], tr[2]},
		"rotation":    []any{r[0], r[1], r[2], r[3]},
		"scale":       []any{sc[0], sc[1], sc[2]},
	})
	for i := range m {
		if math.Abs(m[i]-back[i]) > 1e-6 {
			t.Fatalf("decompose round trip: got %v, want %v", back, m)
		}
	}
}
