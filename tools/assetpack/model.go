package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"image/png"
	"os"
	"path/filepath"

	"github.com/struckchure/earth-two/tools/internal/glb"
)

type digest = [32]byte

type packer struct {
	root   string
	counts map[digest]int
	images map[digest][]byte
}

func newPacker(root string) *packer {
	return &packer{root: root, counts: map[digest]int{}, images: map[digest][]byte{}}
}

func number(v any) (int, error) {
	n, ok := v.(float64)
	if !ok || n < 0 || n != float64(int(n)) {
		return 0, fmt.Errorf("invalid nonnegative integer %v", v)
	}
	return int(n), nil
}

func payload(view any, bin []byte) ([]byte, error) {
	v, ok := view.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("invalid buffer view")
	}
	buffer, err := number(v["buffer"])
	if err != nil || buffer != 0 {
		return nil, fmt.Errorf("only embedded source buffers are supported")
	}
	offset := 0
	if v["byteOffset"] != nil {
		offset, err = number(v["byteOffset"])
		if err != nil {
			return nil, err
		}
	}
	length, err := number(v["byteLength"])
	if err != nil || length <= 0 || offset > len(bin) || length > len(bin)-offset {
		return nil, fmt.Errorf("buffer view outside binary chunk")
	}
	return bin[offset : offset+length], nil
}

func decodeModel(b []byte) (map[string]any, []byte, error) {
	if len(b) < 12 || binary.LittleEndian.Uint32(b[4:8]) != 2 || uint64(binary.LittleEndian.Uint32(b[8:12])) != uint64(len(b)) {
		return nil, nil, fmt.Errorf("invalid GLB version or file length")
	}
	doc, bin, err := glb.Read(b)
	if err != nil {
		return nil, nil, err
	}
	buffers, ok := doc["buffers"].([]any)
	if !ok || len(buffers) != 1 {
		return nil, nil, fmt.Errorf("source must have exactly one embedded buffer")
	}
	buffer, ok := buffers[0].(map[string]any)
	if !ok || buffer["uri"] != nil {
		return nil, nil, fmt.Errorf("external source buffers are unsupported")
	}
	if ext, ok := doc["extensionsUsed"].([]any); ok && len(ext) != 0 {
		return nil, nil, fmt.Errorf("glTF extensions need explicit packing support: %v", ext)
	}
	return doc, bin, nil
}

func (p *packer) count(path string) error {
	b, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	doc, bin, err := decodeModel(b)
	if err != nil {
		return err
	}
	views, _ := doc["bufferViews"].([]any)
	seen := map[digest]bool{}
	for _, v := range views {
		b, err := payload(v, bin)
		if err != nil {
			return err
		}
		hash := sha256.Sum256(b)
		if len(b) >= 4096 && !seen[hash] {
			p.counts[hash]++
			seen[hash] = true
		}
	}
	return nil
}

func optimizePNG(b []byte) ([]byte, error) {
	img, err := png.Decode(bytes.NewReader(b))
	if err != nil {
		return nil, err
	}
	var out bytes.Buffer
	encoder := png.Encoder{CompressionLevel: png.BestCompression}
	if err := encoder.Encode(&out, img); err != nil {
		return nil, err
	}
	if out.Len() < len(b) {
		return out.Bytes(), nil
	}
	return b, nil
}

func (p *packer) shared(name, kind string, b []byte) (string, error) {
	hash := sha256.Sum256(b)
	path := fmt.Sprintf("_packed/%s/%x%s", kind, hash, filepath.Ext(name))
	// Multiple views and models may refer to the same payload.
	target := filepath.Join(p.root, filepath.FromSlash(path))
	if _, err := os.Stat(target); os.IsNotExist(err) {
		if err := writeFile(target, b); err != nil {
			return "", err
		}
	} else if err != nil {
		return "", err
	}
	relative, err := filepath.Rel(filepath.Dir(filepath.FromSlash(name)), filepath.FromSlash(path))
	return filepath.ToSlash(relative), err
}

func (p *packer) model(name string, in []byte) ([]byte, error) {
	doc, bin, err := decodeModel(in)
	if err != nil {
		return nil, err
	}
	views, _ := doc["bufferViews"].([]any)
	images, _ := doc["images"].([]any)
	for _, image := range images {
		im, ok := image.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("invalid image")
		}
		index, err := number(im["bufferView"])
		if err != nil || index >= len(views) {
			return nil, fmt.Errorf("image must refer to an embedded view")
		}
		data, err := payload(views[index], bin)
		if err != nil {
			return nil, err
		}
		originalHash := sha256.Sum256(data)
		ext := ".jpg"
		switch im["mimeType"] {
		case "image/png":
			ext = ".png"
			if cached, ok := p.images[originalHash]; ok {
				data = cached
			} else {
				data, err = optimizePNG(data)
				if err != nil {
					return nil, err
				}
				p.images[originalHash] = data
			}
		case "image/jpeg":
		default:
			return nil, fmt.Errorf("unsupported image MIME type %v", im["mimeType"])
		}
		// Duplicate textures use a single external image; unique textures stay
		// embedded, including losslessly recompressed PNGs.
		if p.counts[originalHash] > 1 {
			uri, err := p.shared(name+ext, "textures", data)
			if err != nil {
				return nil, err
			}
			delete(im, "bufferView")
			im["uri"] = uri
		} else {
			p.images[originalHash] = data
		}
	}
	// Keep all views still referenced by accessors (including sparse data),
	// images, or other core structures. No accessor or primitive is reordered.
	referenced := map[int]bool{}
	if err := walk(doc, func(key string, value any) error {
		if key == "bufferView" {
			index, err := number(value)
			if err != nil || index >= len(views) {
				return fmt.Errorf("invalid view reference %v", value)
			}
			referenced[index] = true
		}
		return nil
	}); err != nil {
		return nil, err
	}
	var packed []byte
	newViews := []any{}
	buffers := []any{map[string]any{}}
	remap := map[int]int{}
	localOffsets := map[digest]int{}
	externalBuffers := map[digest]int{}
	for i, view := range views {
		if !referenced[i] {
			continue
		}
		data, err := payload(view, bin)
		if err != nil {
			return nil, err
		}
		hash := sha256.Sum256(data)
		v := view.(map[string]any)
		remap[i] = len(newViews)
		newViews = append(newViews, v)
		if image, ok := p.images[hash]; ok {
			data = image
		}
		// Geometry shared by different models can use ordinary glTF external
		// buffers. Each payload has its own file, so loading one model doesn't
		// read unrelated models' geometry into memory.
		if p.counts[hash] > 1 && p.images[hash] == nil {
			at, ok := externalBuffers[hash]
			if !ok {
				uri, err := p.shared(name+".bin", "buffers", data)
				if err != nil {
					return nil, err
				}
				at = len(buffers)
				externalBuffers[hash] = at
				buffers = append(buffers, map[string]any{"uri": uri, "byteLength": len(data)})
			}
			v["buffer"], v["byteOffset"], v["byteLength"] = at, 0, len(data)
		} else {
			hash = sha256.Sum256(data)
			at, ok := localOffsets[hash]
			if !ok {
				for len(packed)%4 != 0 {
					packed = append(packed, 0)
				}
				at = len(packed)
				localOffsets[hash] = at
				packed = append(packed, data...)
			}
			v["buffer"], v["byteOffset"], v["byteLength"] = 0, at, len(data)
		}
	}
	var rewrite func(any)
	rewrite = func(value any) {
		switch v := value.(type) {
		case map[string]any:
			for key, item := range v {
				if key == "bufferView" {
					index, _ := number(item)
					v[key] = remap[index]
				} else {
					rewrite(item)
				}
			}
		case []any:
			for _, item := range v {
				rewrite(item)
			}
		}
	}
	rewrite(doc)
	if len(packed) == 0 {
		packed = make([]byte, 4)
	}
	for len(packed)%4 != 0 {
		packed = append(packed, 0)
	}
	buffers[0].(map[string]any)["byteLength"] = len(packed)
	doc["bufferViews"], doc["buffers"] = newViews, buffers
	return encodeModel(doc, packed)
}

func encodeModel(doc map[string]any, bin []byte) ([]byte, error) {
	js, err := json.Marshal(doc)
	if err != nil {
		return nil, err
	}
	for len(js)%4 != 0 {
		js = append(js, ' ')
	}
	out := new(bytes.Buffer)
	for _, n := range []uint32{0x46546c67, 2, uint32(28 + len(js) + len(bin)), uint32(len(js)), 0x4e4f534a} {
		_ = binary.Write(out, binary.LittleEndian, n)
	}
	out.Write(js)
	_ = binary.Write(out, binary.LittleEndian, uint32(len(bin)))
	_ = binary.Write(out, binary.LittleEndian, uint32(0x004e4942))
	out.Write(bin)
	return out.Bytes(), nil
}
