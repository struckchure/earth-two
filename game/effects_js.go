//go:build js

package game

import rl "github.com/gen2brain/raylib-go/raylib"

// The GLSL version the particles' shader is written for, in the browser
// (WebGL 2), which also wants a default precision.
const (
	effectsVertexHeader   = "#version 300 es\n"
	effectsFragmentHeader = "#version 300 es\nprecision highp float;\n"
)

// The web wrapper draws the C-side mesh stored at upload, whose counts
// stay fixed. It uses the collapsed tail already cleared by quads.
func particleDrawMesh(mesh rl.Mesh, _ int) rl.Mesh { return mesh }
