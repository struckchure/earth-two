package main

import (
	"bytes"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/struckchure/earth-two/tools/internal/glb"
)

func fixtureModel(t *testing.T) []byte {
	t.Helper()
	img := image.NewNRGBA(image.Rect(0, 0, 64, 64))
	seed := uint32(42)
	for i := range img.Pix {
		seed = seed*1664525 + 1013904223
		img.Pix[i] = byte(seed >> 24)
	}
	var encoded bytes.Buffer
	encoder := png.Encoder{CompressionLevel: png.NoCompression}
	if err := encoder.Encode(&encoded, img); err != nil {
		t.Fatal(err)
	}
	geometry := make([]byte, 4096)
	for i := range geometry {
		geometry[i] = byte(i % 251)
	}
	doc := map[string]any{
		"asset": map[string]any{"version": "2.0", "extras": map[string]any{"painted": true}},
		"bufferViews": []any{
			map[string]any{"buffer": 0, "byteOffset": 0, "byteLength": len(geometry)},
			map[string]any{"buffer": 0, "byteOffset": len(geometry), "byteLength": encoded.Len()},
		},
		"accessors": []any{map[string]any{"bufferView": 0, "componentType": 5126, "type": "SCALAR", "count": 1024}},
		"images":    []any{map[string]any{"bufferView": 1, "mimeType": "image/png", "name": "alpha"}},
		"nodes":     []any{map[string]any{"name": "first"}, map[string]any{"name": "second"}},
	}
	b, err := glb.Write(doc, append(geometry, encoded.Bytes()...))
	if err != nil {
		t.Fatal(err)
	}
	return b
}

func viewBytes(t *testing.T, name string, doc map[string]any, bin []byte, index int) []byte {
	t.Helper()
	v := doc["bufferViews"].([]any)[index].(map[string]any)
	buffer := int(v["buffer"].(float64))
	if buffer != 0 {
		uri := doc["buffers"].([]any)[buffer].(map[string]any)["uri"].(string)
		var err error
		bin, err = os.ReadFile(filepath.Join(filepath.Dir(name), filepath.FromSlash(uri)))
		if err != nil {
			t.Fatal(err)
		}
	}
	off := int(v["byteOffset"].(float64))
	n := int(v["byteLength"].(float64))
	return bin[off : off+n]
}

func assertModelPreserved(t *testing.T, original []byte, packed string) {
	t.Helper()
	source, srcBin, err := glb.Read(original)
	if err != nil {
		t.Fatal(err)
	}
	b, err := os.ReadFile(packed)
	if err != nil {
		t.Fatal(err)
	}
	doc, bin, err := glb.Read(b)
	if err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{"asset", "nodes"} {
		if !reflect.DeepEqual(source[field], doc[field]) {
			t.Fatalf("%s changed", field)
		}
	}
	src := viewBytes(t, "", source, srcBin, 0)
	index := int(doc["accessors"].([]any)[0].(map[string]any)["bufferView"].(float64))
	if !bytes.Equal(src, viewBytes(t, packed, doc, bin, index)) {
		t.Fatal("geometry changed")
	}
	im := doc["images"].([]any)[0].(map[string]any)
	imageData, err := os.ReadFile(filepath.Join(filepath.Dir(packed), filepath.FromSlash(im["uri"].(string))))
	if err != nil {
		t.Fatal(err)
	}
	before, err := png.Decode(bytes.NewReader(viewBytes(t, "", source, srcBin, 1)))
	if err != nil {
		t.Fatal(err)
	}
	after, err := png.Decode(bytes.NewReader(imageData))
	if err != nil {
		t.Fatal(err)
	}
	if before.Bounds() != after.Bounds() {
		t.Fatal("image dimensions changed")
	}
	for y := 0; y < before.Bounds().Dy(); y++ {
		for x := 0; x < before.Bounds().Dx(); x++ {
			a := color.NRGBA64Model.Convert(before.At(x, y))
			b := color.NRGBA64Model.Convert(after.At(x, y))
			if a != b {
				t.Fatalf("pixel changed at %d,%d", x, y)
			}
		}
	}
}

func fixture(t *testing.T) (string, string, string) {
	t.Helper()
	dir := t.TempDir()
	source := filepath.Join(dir, "source")
	output := filepath.Join(dir, "packed")
	original := fixtureModel(t)
	for _, name := range []string{"car", "wheel", "brush", "unused"} {
		if err := writeFile(filepath.Join(source, "world", name+".glb"), original); err != nil {
			t.Fatal(err)
		}
	}
	if err := writeJSON(filepath.Join(source, "world/world.json"), map[string]any{"pieces": map[string]any{
		"car":    map[string]any{"model": "world/car.glb", "vehicle": map[string]any{"wheels": []any{map[string]any{"piece": "wheel"}}}},
		"wheel":  map[string]any{"model": "world/wheel.glb"},
		"brush":  map[string]any{"model": "world/brush.glb"},
		"unused": map[string]any{"model": "world/unused.glb"},
	}}); err != nil {
		t.Fatal(err)
	}
	if err := writeJSON(filepath.Join(source, "world/landfall.json"), map[string]any{"pieces": []any{map[string]any{"piece": "car"}}}); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(source, "sounds"), 0755); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"step_0.ogg", "step_1.ogg", "unused.ogg"} {
		if err := writeFile(filepath.Join(source, "sounds", name), []byte(name)); err != nil {
			t.Fatal(err)
		}
	}
	code := filepath.Join(dir, "code")
	if err := writeFile(filepath.Join(code, "runtime.go"), []byte("package fixture\nvar scatter = []string{\"brush\"}\nvar cues = []string{\"step\"}\n// unused\n")); err != nil {
		t.Fatal(err)
	}
	if err := writeFile(filepath.Join(code, "runtime_test.go"), []byte("package fixture\nvar testPiece = \"unused\"\n")); err != nil {
		t.Fatal(err)
	}
	config := filepath.Join(dir, "manifest.json")
	if err := writeJSON(config, manifest{World: "world/world.json", Layouts: []string{"world/landfall.json"}, Code: []string{code}}); err != nil {
		t.Fatal(err)
	}
	return source, output, config
}

func TestPackReachabilityAndLosslessSharing(t *testing.T) {
	source, output, config := fixture(t)
	before, _ := os.ReadFile(filepath.Join(source, "world/car.glb"))
	r, err := pack(source, output, config)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(r.Pieces, []string{"brush", "car", "wheel"}) {
		t.Fatalf("pieces %v", r.Pieces)
	}
	for _, excluded := range []string{"world/unused.glb", "sounds/unused.ogg"} {
		if _, err := os.Stat(filepath.Join(output, excluded)); !os.IsNotExist(err) {
			t.Fatalf("included %s", excluded)
		}
	}
	for _, name := range []string{"car", "wheel", "brush"} {
		assertModelPreserved(t, before, filepath.Join(output, "world", name+".glb"))
	}
	after, _ := os.ReadFile(filepath.Join(source, "world/car.glb"))
	if !bytes.Equal(before, after) {
		t.Fatal("source changed")
	}
	var catalog map[string]any
	if err := readJSON(filepath.Join(output, "world/world.json"), &catalog); err != nil {
		t.Fatal(err)
	}
	if len(catalog["pieces"].(map[string]any)) != 3 {
		t.Fatal("catalogue was not pruned")
	}
	// Repacking replaces the old directory, removes stale files, and gives
	// byte-identical output (including external URIs and the report).
	snapshot := map[string][]byte{}
	for _, f := range r.Files {
		snapshot[f.Path], _ = os.ReadFile(filepath.Join(output, filepath.FromSlash(f.Path)))
	}
	if err := writeFile(filepath.Join(output, "stale.bin"), []byte("stale")); err != nil {
		t.Fatal(err)
	}
	again, err := pack(source, output, config)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(r, again) {
		t.Fatal("report is not deterministic")
	}
	for path, b := range snapshot {
		now, _ := os.ReadFile(filepath.Join(output, filepath.FromSlash(path)))
		if !bytes.Equal(b, now) {
			t.Fatalf("nondeterministic %s", path)
		}
	}
	if _, err := os.Stat(filepath.Join(output, "stale.bin")); !os.IsNotExist(err) {
		t.Fatal("stale file retained")
	}
	if r.PackedBytes >= r.SelectedBytes {
		t.Fatal("sharing did not reduce size")
	}
}

func TestPackFailsWithoutReplacingPreviousOutput(t *testing.T) {
	source, output, config := fixture(t)
	if _, err := pack(source, output, config); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(filepath.Join(source, "world/wheel.glb")); err != nil {
		t.Fatal(err)
	}
	if _, err := pack(source, output, config); err == nil {
		t.Fatal("missing wheel accepted")
	}
	if _, err := os.Stat(filepath.Join(output, "world/wheel.glb")); err != nil {
		t.Fatal("previous output lost")
	}
}

func TestUnsafePathsAndOutput(t *testing.T) {
	source, output, config := fixture(t)
	if _, err := pack(source, source, config); err == nil {
		t.Fatal("overlapping directories accepted")
	}
	if err := os.Mkdir(output, 0755); err != nil {
		t.Fatal(err)
	}
	if _, err := pack(source, output, config); err == nil {
		t.Fatal("unowned directory replaced")
	}
	for _, path := range []string{"../secret", "/absolute", "world/../secret", "world\\car.glb"} {
		if _, err := assetPath(source, path); err == nil {
			t.Fatalf("unsafe path accepted: %s", path)
		}
	}
}

func TestMalformedModel(t *testing.T) {
	p := newPacker(t.TempDir())
	original := fixtureModel(t)
	doc, bin, _ := glb.Read(original)
	doc["bufferViews"].([]any)[0].(map[string]any)["byteLength"] = len(bin) + 1
	bad, _ := glb.Write(doc, bin)
	if _, err := p.model("world/bad.glb", bad); err == nil {
		t.Fatal("out-of-bounds view accepted")
	}
	doc["bufferViews"].([]any)[0].(map[string]any)["byteLength"] = 4096
	doc["extensionsUsed"] = []any{"KHR_draco_mesh_compression"}
	bad, _ = glb.Write(doc, bin)
	if _, err := p.model("world/bad.glb", bad); err == nil {
		t.Fatal("unknown extension accepted")
	}
}

func TestPNGPreservesTransparentRGB(t *testing.T) {
	// Hidden RGB under zero alpha must survive as well as visible colour.
	img := image.NewNRGBA(image.Rect(0, 0, 16, 16))
	img.SetNRGBA(0, 0, color.NRGBA{R: 255, G: 127, B: 32, A: 0})
	var input bytes.Buffer
	enc := png.Encoder{CompressionLevel: png.NoCompression}
	if err := enc.Encode(&input, img); err != nil {
		t.Fatal(err)
	}
	b, err := optimizePNG(input.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	out, err := png.Decode(bytes.NewReader(b))
	if err != nil {
		t.Fatal(err)
	}
	if color.NRGBAModel.Convert(out.At(0, 0)) != img.NRGBAAt(0, 0) {
		t.Fatal("transparent RGB changed")
	}
}
