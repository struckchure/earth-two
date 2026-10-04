package main

import (
	"bytes"
	"errors"
	"image"
	"image/color"
	"image/jpeg"
	"image/png"
	"testing"

	"github.com/struckchure/earth-two/tools/internal/glb"
)

// halves is an image whose left half is one colour and right half another,
// with noise of the given size on both.
func halves(noise int) *image.NRGBA {
	im := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := range 32 {
		for x := range 32 {
			v := 60 + (x*7+y*13)%(noise+1)
			if x >= 16 {
				v += 120
			}
			im.SetNRGBA(x, y, color.NRGBA{uint8(v), uint8(v), uint8(v), 255})
		}
	}
	return im
}

func TestPaintKeepsFlatColourAndEdges(t *testing.T) {
	out := paint(halves(0), 4)
	left, right := out.NRGBAAt(15, 16), out.NRGBAAt(16, 16)
	if left != out.NRGBAAt(3, 3) || right != out.NRGBAAt(28, 28) {
		t.Errorf("flat halves aren't flat: %v %v at the edge, %v %v away from it", left, right, out.NRGBAAt(3, 3), out.NRGBAAt(28, 28))
	}
	if int(right.R)-int(left.R) < 110 {
		t.Errorf("the edge was blurred: %v beside %v", left, right)
	}
}

func TestPaintSmoothsNoise(t *testing.T) {
	spread := func(im *image.NRGBA) int {
		lo, hi := 255, 0
		for y := 8; y < 24; y++ {
			for x := 2; x < 10; x++ {
				v := int(im.NRGBAAt(x, y).R)
				lo, hi = min(lo, v), max(hi, v)
			}
		}
		return hi - lo
	}
	in := halves(20)
	if before, after := spread(in), spread(paint(in, 4)); after*2 > before {
		t.Errorf("noise went from a spread of %d to %d", before, after)
	}
}

func TestPaintKeepsAlpha(t *testing.T) {
	in := halves(0)
	for y := range 32 {
		in.Pix[in.PixOffset(20, y)+3] = 0
	}
	out := paint(in, 4)
	if out.NRGBAAt(20, 5).A != 0 || out.NRGBAAt(19, 5).A != 255 {
		t.Error("alpha changed")
	}
}

func TestPaintGLB(t *testing.T) {
	var texture bytes.Buffer
	if err := png.Encode(&texture, halves(20)); err != nil {
		t.Fatal(err)
	}
	// An image between two other views, to see them keep their bytes.
	bin := append([]byte{1, 2, 3, 4}, texture.Bytes()...)
	for len(bin)%4 != 0 {
		bin = append(bin, 0)
	}
	tail := len(bin)
	bin = append(bin, 5, 6, 7, 8)
	in, err := glb.Write(map[string]any{
		"asset": map[string]any{"version": "2.0"},
		"bufferViews": []any{
			map[string]any{"buffer": 0, "byteOffset": 0, "byteLength": 4},
			map[string]any{"buffer": 0, "byteOffset": 4, "byteLength": texture.Len()},
			map[string]any{"buffer": 0, "byteOffset": tail, "byteLength": 4},
		},
		"images": []any{map[string]any{"bufferView": 1, "mimeType": "image/png"}},
	}, bin)
	if err != nil {
		t.Fatal(err)
	}

	out, err := paintGLB(in)
	if err != nil {
		t.Fatal(err)
	}
	doc, packed, err := glb.Read(out)
	if err != nil {
		t.Fatal(err)
	}
	view := func(i int) []byte {
		start, end, err := span(doc["bufferViews"].([]any)[i], len(packed))
		if err != nil {
			t.Fatal(err)
		}
		return packed[start:end]
	}
	if !bytes.Equal(view(0), []byte{1, 2, 3, 4}) || !bytes.Equal(view(2), []byte{5, 6, 7, 8}) {
		t.Error("the other buffer views changed")
	}
	if im, err := png.Decode(bytes.NewReader(view(1))); err != nil || im.Bounds().Dx() != 32 {
		t.Errorf("the painted image doesn't decode: %v", err)
	}
	if bytes.Equal(view(1), texture.Bytes()) {
		t.Error("the image wasn't painted")
	}
	if _, err := paintGLB(out); !errors.Is(err, errPainted) {
		t.Errorf("painting it again: %v, want errPainted", err)
	}
}

func TestPaintJPEG(t *testing.T) {
	var in bytes.Buffer
	if err := jpeg.Encode(&in, halves(20), nil); err != nil {
		t.Fatal(err)
	}
	out, err := paintJPEG(in.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := jpeg.Decode(bytes.NewReader(out)); err != nil {
		t.Errorf("the painted image doesn't decode: %v", err)
	}
	if _, err := paintJPEG(out); !errors.Is(err, errPainted) {
		t.Errorf("painting it again: %v, want errPainted", err)
	}
}
