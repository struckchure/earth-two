package main

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/struckchure/earth-two/tools/internal/glb"
)

// Opt-in integration check against a real build: compare every retained
// model's complete document, geometry, skinning and animation payloads, and
// decoded texture pixels. This is separate from the small fixture tests.
func TestPackedRepository(t *testing.T) {
	output := os.Getenv("EARTH_TWO_PACK_CHECK")
	if output == "" {
		t.Skip("set EARTH_TWO_PACK_CHECK to the packed assets directory")
	}
	root, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	var r report
	if err := readJSON(filepath.Join(filepath.Dir(output), "asset-report.json"), &r); err != nil {
		t.Fatal(err)
	}
	for _, name := range r.Included {
		if filepath.Ext(name) != ".glb" {
			continue
		}
		t.Run(name, func(t *testing.T) {
			source := canonicalModel(t, filepath.Join(root, "assets", filepath.FromSlash(name)))
			packed := canonicalModel(t, filepath.Join(output, filepath.FromSlash(name)))
			if !reflect.DeepEqual(source, packed) {
				t.Fatal("model semantics changed")
			}
		})
	}
}

func canonicalModel(t *testing.T, path string) map[string]any {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	doc, bin, err := glb.Read(b)
	if err != nil {
		t.Fatal(err)
	}
	views := doc["bufferViews"].([]any)
	images := doc["images"].([]any)
	for _, v := range images {
		im := v.(map[string]any)
		var encoded []byte
		if uri, ok := im["uri"].(string); ok {
			encoded, err = os.ReadFile(filepath.Join(filepath.Dir(path), filepath.FromSlash(uri)))
			if err != nil {
				t.Fatal(err)
			}
		} else {
			encoded = viewBytes(t, path, doc, bin, int(im["bufferView"].(float64)))
		}
		delete(im, "uri")
		delete(im, "bufferView")
		// JPEG bytes are untouched. PNG bytes may change but every decoded
		// channel, including RGB under fully transparent pixels, must match.
		if im["mimeType"] == "image/png" {
			img, err := png.Decode(bytes.NewReader(encoded))
			if err != nil {
				t.Fatal(err)
			}
			hash := sha256.New()
			fmt.Fprintf(hash, "%v", img.Bounds())
			for y := img.Bounds().Min.Y; y < img.Bounds().Max.Y; y++ {
				for x := img.Bounds().Min.X; x < img.Bounds().Max.X; x++ {
					c := color.NRGBA64Model.Convert(img.At(x, y)).(color.NRGBA64)
					hash.Write([]byte{byte(c.R >> 8), byte(c.R), byte(c.G >> 8), byte(c.G), byte(c.B >> 8), byte(c.B), byte(c.A >> 8), byte(c.A)})
				}
			}
			im["pixels"] = fmt.Sprintf("%x", hash.Sum(nil))
		} else {
			im["pixels"] = fmt.Sprintf("%x", sha256.Sum256(encoded))
		}
	}
	var normalize func(any)
	normalize = func(value any) {
		switch v := value.(type) {
		case map[string]any:
			for key, item := range v {
				if key == "bufferView" {
					index := int(item.(float64))
					view := views[index].(map[string]any)
					semantic := map[string]any{}
					for k, val := range view {
						if k != "buffer" && k != "byteOffset" && k != "byteLength" {
							semantic[k] = val
						}
					}
					semantic["data"] = fmt.Sprintf("%x", sha256.Sum256(viewBytes(t, path, doc, bin, index)))
					v[key] = semantic
				} else {
					normalize(item)
				}
			}
		case []any:
			for _, item := range v {
				normalize(item)
			}
		}
	}
	normalize(doc)
	delete(doc, "buffers")
	delete(doc, "bufferViews")
	return doc
}
