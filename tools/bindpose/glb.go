package main

import (
	"errors"

	"github.com/struckchure/earth-two/tools/internal/glb"
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

func readGLB(b []byte) (obj, []byte, error) { return glb.Read(b) }

func (g *gltf) writeGLB() ([]byte, error) { return glb.Write(g.doc, g.bin) }

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
