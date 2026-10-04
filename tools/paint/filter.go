package main

import (
	"image"
	"math"
	"runtime"
	"sync"
)

// sectors is how many directions the filter looks in around each pixel, and
// sharpness how strongly it prefers the evenest of them.
const (
	sectors   = 8
	sharpness = 8
)

// paint is src through a generalized Kuwahara filter of the given radius,
// then with its colours lifted: each pixel becomes the mean of the evenest
// directions around it, which flattens a photograph into patches of colour
// with soft edges between them while keeping the edges it had. Alpha is
// kept, and what's transparent doesn't colour what's around it.
func paint(src *image.NRGBA, radius int) *image.NRGBA {
	b := src.Bounds()
	w, h := b.Dx(), b.Dy()
	dst := image.NewNRGBA(b)

	// The neighbourhood: each offset's sector and weight. The centre counts
	// in every sector.
	type tap struct {
		dx, dy, sector int
		weight         float64
	}
	var taps []tap
	sigma := float64(radius) / 2
	for dy := -radius; dy <= radius; dy++ {
		for dx := -radius; dx <= radius; dx++ {
			d2 := float64(dx*dx + dy*dy)
			if d2 > float64(radius*radius) || d2 == 0 {
				continue
			}
			angle := math.Atan2(float64(dy), float64(dx)) + math.Pi
			sector := int(angle/(2*math.Pi)*sectors) % sectors
			taps = append(taps, tap{dx, dy, sector, math.Exp(-d2 / (2 * sigma * sigma))})
		}
	}

	rows := make(chan int)
	var wg sync.WaitGroup
	for range runtime.NumCPU() {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for y := range rows {
				for x := range w {
					at := src.PixOffset(b.Min.X+x, b.Min.Y+y)
					centre := src.Pix[at : at+4 : at+4]
					out := dst.Pix[at : at+4 : at+4]
					copy(out, centre)
					if centre[3] == 0 {
						continue
					}
					// Each sector's weight, sum and sum of squares.
					var n [sectors]float64
					var sum, sq [sectors][3]float64
					add := func(s int, p []uint8, weight float64) {
						weight *= float64(p[3]) / 255
						n[s] += weight
						for c := range 3 {
							v := float64(p[c])
							sum[s][c] += weight * v
							sq[s][c] += weight * v * v
						}
					}
					for s := range sectors {
						add(s, centre, 1)
					}
					for _, t := range taps {
						px, py := min(max(x+t.dx, 0), w-1), min(max(y+t.dy, 0), h-1)
						o := src.PixOffset(b.Min.X+px, b.Min.Y+py)
						add(t.sector, src.Pix[o:o+4:o+4], t.weight)
					}
					var total float64
					var mean [3]float64
					for s := range sectors {
						var variance float64
						for c := range 3 {
							m := sum[s][c] / n[s]
							variance += sq[s][c]/n[s] - m*m
						}
						// An even sector weighs about 1, and an uneven one next
						// to nothing.
						weight := 1 / (1 + math.Pow(math.Sqrt(max(variance, 0)), sharpness))
						total += weight
						for c := range 3 {
							mean[c] += weight * sum[s][c] / n[s]
						}
					}
					r, g, bl := lift(mean[0]/total/255, mean[1]/total/255, mean[2]/total/255)
					out[0], out[1], out[2] = to8(r), to8(g), to8(bl)
				}
			}
		}()
	}
	for y := range h {
		rows <- y
	}
	close(rows)
	wg.Wait()
	return dst
}

// saturation and contrast are how much paint lifts the colours.
const (
	saturation = 1.15
	contrast   = 1.06
)

// lift saturates a colour (in 0..1) and pushes it away from mid grey.
func lift(r, g, b float64) (float64, float64, float64) {
	luma := 0.299*r + 0.587*g + 0.114*b
	each := func(v float64) float64 {
		v = luma + (v-luma)*saturation
		return (v-0.5)*contrast + 0.5
	}
	return each(r), each(g), each(b)
}

func to8(v float64) uint8 {
	return uint8(math.Round(min(max(v, 0), 1) * 255))
}

// radiusFor is the brush for an image of the given size: wider on bigger
// images, and small enough on little ones (eyes) to leave their detail.
func radiusFor(size image.Point) int {
	return min(max(int(math.Round(float64(max(size.X, size.Y))/200)), 2), 6)
}
