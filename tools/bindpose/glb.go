package main

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
)

type obj = map[string]any

// gltf is a parsed .glb: its JSON document and its binary chunk.
type gltf struct {
	doc obj
	bin []byte
}

func (g *gltf) list(key string) []any {
	l, _ := g.doc[key].([]any)
	return l
}

func (g *gltf) accessorBytes(i int) (acc obj, data []byte, stride int, err error) {
	acc = g.list("accessors")[i].(obj)
	if _, ok := acc["sparse"]; ok {
		return nil, nil, 0, errors.New("sparse accessors aren't supported")
	}
	view := g.list("bufferViews")[num(acc["bufferView"])].(obj)
	if num(view["buffer"]) != 0 {
		return nil, nil, 0, errors.New("only the GLB buffer is supported")
	}
	start := num(view["byteOffset"]) + num(acc["byteOffset"])
	end := num(view["byteOffset"]) + num(view["byteLength"])
	return acc, g.bin[start:end], num(view["byteStride"]), nil
}

func readGLB(b []byte) (obj, []byte, error) {
	if len(b) < 20 || string(b[:4]) != "glTF" {
		return nil, nil, errors.New("not a .glb file")
	}
	var doc obj
	var bin []byte
	for off := 12; off+8 <= len(b); {
		n := int(binary.LittleEndian.Uint32(b[off:]))
		kind := string(b[off+4 : off+8])
		chunk := b[off+8 : off+8+n]
		switch kind {
		case "JSON":
			if err := json.Unmarshal(chunk, &doc); err != nil {
				return nil, nil, err
			}
		case "BIN\x00":
			bin = append([]byte(nil), chunk...)
		}
		off += 8 + n
	}
	if doc == nil {
		return nil, nil, errors.New("no JSON chunk")
	}
	return doc, bin, nil
}

func (g *gltf) writeGLB() ([]byte, error) {
	for len(g.bin)%4 != 0 {
		g.bin = append(g.bin, 0)
	}
	g.doc["buffers"] = []any{obj{"byteLength": len(g.bin)}}
	js, err := json.Marshal(g.doc)
	if err != nil {
		return nil, err
	}
	for len(js)%4 != 0 {
		js = append(js, ' ')
	}
	out := new(bytes.Buffer)
	le := binary.LittleEndian
	binary.Write(out, le, [3]uint32{0x46546C67, 2, uint32(12 + 8 + len(js) + 8 + len(g.bin))})
	binary.Write(out, le, [2]uint32{uint32(len(js)), 0x4E4F534A})
	out.Write(js)
	binary.Write(out, le, [2]uint32{uint32(len(g.bin)), 0x004E4942})
	out.Write(g.bin)
	return out.Bytes(), nil
}

func num(v any) int {
	f, _ := v.(float64)
	if i, ok := v.(int); ok {
		return i
	}
	return int(f)
}

func ints(v any) []int {
	l, _ := v.([]any)
	out := make([]int, len(l))
	for i, x := range l {
		out[i] = num(x)
	}
	return out
}

func floats(v any, def ...float64) []float64 {
	l, ok := v.([]any)
	if !ok {
		return def
	}
	out := make([]float64, len(l))
	for i, x := range l {
		out[i], _ = x.(float64)
	}
	return out
}
