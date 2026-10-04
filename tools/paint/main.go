// Paint gives the characters' textures a painted look: it runs every image
// in the .glb files and every .jpg it's given (or finds under the
// directories it's given) through the filter in filter.go, in place.
//
// What it paints it marks (a .glb in asset.extras, a .jpg in a comment), and
// it leaves marked files alone, so running it again changes nothing.
//
//	go run ./tools/paint assets/characters
package main

import (
	"bytes"
	"errors"
	"fmt"
	"image"
	"image/draw"
	"image/jpeg"
	"image/png"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/struckchure/earth-two/tools/internal/glb"
)

// jpegQuality matches what tools/makehuman exports.
const jpegQuality = 85

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: paint <file or directory>...")
		os.Exit(2)
	}
	for _, root := range os.Args[1:] {
		err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return err
			}
			var painter func([]byte) ([]byte, error)
			switch strings.ToLower(filepath.Ext(path)) {
			case ".glb":
				painter = paintGLB
			case ".jpg", ".jpeg":
				painter = paintJPEG
			default:
				return nil
			}
			in, err := os.ReadFile(path)
			if err != nil {
				return err
			}
			out, err := painter(in)
			if errors.Is(err, errPainted) {
				return nil
			}
			if err != nil {
				return fmt.Errorf("%s: %w", path, err)
			}
			fmt.Println("painted", path)
			return os.WriteFile(path, out, 0o644)
		})
		if err != nil {
			fmt.Fprintln(os.Stderr, "paint:", err)
			os.Exit(1)
		}
	}
}

// errPainted says a file has been painted already.
var errPainted = errors.New("already painted")

// paintGLB paints the images in a .glb file's binary chunk.
func paintGLB(in []byte) ([]byte, error) {
	doc, bin, err := glb.Read(in)
	if err != nil {
		return nil, err
	}
	asset, _ := doc["asset"].(map[string]any)
	if asset == nil {
		asset = map[string]any{"version": "2.0"}
		doc["asset"] = asset
	}
	extras, _ := asset["extras"].(map[string]any)
	if extras["painted"] == true {
		return nil, errPainted
	}
	if extras == nil {
		extras = map[string]any{}
		asset["extras"] = extras
	}
	extras["painted"] = true

	views, _ := doc["bufferViews"].([]any)
	painted := map[int][]byte{} // by buffer view
	images, _ := doc["images"].([]any)
	for i, im := range images {
		im := im.(map[string]any)
		index, ok := im["bufferView"].(float64)
		if !ok {
			return nil, fmt.Errorf("image %d isn't in the file", i)
		}
		start, end, err := span(views[int(index)], len(bin))
		if err != nil {
			return nil, err
		}
		mime, _ := im["mimeType"].(string)
		if painted[int(index)], err = paintImage(bin[start:end], mime == "image/png", false); err != nil {
			return nil, fmt.Errorf("image %d: %w", i, err)
		}
	}

	// Repack the buffer, as the images have changed size.
	var packed []byte
	for i, v := range views {
		start, end, err := span(v, len(bin))
		if err != nil {
			return nil, err
		}
		data := bin[start:end]
		if p, ok := painted[i]; ok {
			data = p
		}
		for len(packed)%4 != 0 {
			packed = append(packed, 0)
		}
		v := v.(map[string]any)
		v["byteOffset"] = len(packed)
		v["byteLength"] = len(data)
		packed = append(packed, data...)
	}
	return glb.Write(doc, packed)
}

// span is where a buffer view lies in the binary chunk.
func span(view any, size int) (start, end int, err error) {
	v, _ := view.(map[string]any)
	if b, _ := v["buffer"].(float64); b != 0 {
		return 0, 0, errors.New("only the GLB buffer is supported")
	}
	offset, _ := v["byteOffset"].(float64)
	length, _ := v["byteLength"].(float64)
	start, end = int(offset), int(offset+length)
	if start < 0 || end > size || start > end {
		return 0, 0, errors.New("buffer view outside the buffer")
	}
	return start, end, nil
}

func paintJPEG(in []byte) ([]byte, error) {
	if bytes.Contains(in, jpegMark) {
		return nil, errPainted
	}
	return paintImage(in, false, true)
}

// jpegMark is the comment segment that marks a painted .jpg, which goes
// right after the start-of-image marker.
var jpegMark = []byte("\xff\xfe\x00\x14earth-two: painted")

// paintImage paints an encoded image, and encodes it as a PNG or a JPEG.
func paintImage(in []byte, asPNG, mark bool) ([]byte, error) {
	decoded, _, err := image.Decode(bytes.NewReader(in))
	if err != nil {
		return nil, err
	}
	src := image.NewNRGBA(decoded.Bounds())
	draw.Draw(src, src.Bounds(), decoded, decoded.Bounds().Min, draw.Src)
	dst := paint(src, radiusFor(src.Bounds().Size()))

	out := new(bytes.Buffer)
	if asPNG {
		err = png.Encode(out, dst)
		return out.Bytes(), err
	}
	if err := jpeg.Encode(out, dst, &jpeg.Options{Quality: jpegQuality}); err != nil {
		return nil, err
	}
	if !mark {
		return out.Bytes(), nil
	}
	b := out.Bytes()
	return append(append(append([]byte(nil), b[:2]...), jpegMark...), b[2:]...), nil
}
